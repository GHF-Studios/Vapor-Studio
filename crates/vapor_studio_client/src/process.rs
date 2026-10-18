use crate::transport::{encode_frame, read_frame};
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError},
    thread,
    time::{Duration, Instant},
};
use vapor_studio_protocol::{
    LegacyStorageSeed, PROTOCOL_VERSION, ProtocolFrame, RequestId, StudioAction, StudioEvent,
    StudioRequest, StudioRevision, StudioSnapshot,
};

const START_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct ServiceBootstrap {
    pub application_executable: PathBuf,
    pub workspace_root: Option<PathBuf>,
    pub active_profile_id: Option<String>,
    pub fixture_mode: bool,
    pub legacy_storage: LegacyStorageSeed,
    pub state_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    Spawn(String),
    Protocol(String),
    ServiceExited(Option<i32>),
    Timeout(&'static str),
    Io(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(message) => write!(formatter, "could not start Studio service: {message}"),
            Self::Protocol(message) => {
                write!(formatter, "Studio service protocol error: {message}")
            }
            Self::ServiceExited(code) => {
                write!(formatter, "Studio service exited unexpectedly ({code:?})")
            }
            Self::Timeout(phase) => write!(formatter, "Studio service timed out during {phase}"),
            Self::Io(message) => write!(formatter, "Studio service I/O failed: {message}"),
        }
    }
}

impl std::error::Error for ClientError {}

pub struct StudioClient {
    child: Child,
    writer: Option<BufWriter<ChildStdin>>,
    incoming: Receiver<Result<ProtocolFrame, ClientError>>,
    next_request: u64,
    latest_snapshot: StudioSnapshot,
}

impl StudioClient {
    pub fn spawn(service_path: &Path, bootstrap: ServiceBootstrap) -> Result<Self, ClientError> {
        let mut command = Command::new(service_path);
        command
            .arg("--stdio")
            .arg("--application-executable")
            .arg(&bootstrap.application_executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(workspace) = bootstrap.workspace_root {
            command.arg("--workspace").arg(workspace);
        }
        if let Some(profile_id) = bootstrap.active_profile_id {
            command.arg("--active-profile").arg(profile_id);
        }
        if let Some(state_directory) = bootstrap.state_directory {
            command.arg("--state-dir").arg(state_directory);
        }
        if bootstrap.fixture_mode {
            command.arg("--fixture");
        }
        let mut child = command
            .spawn()
            .map_err(|error| ClientError::Spawn(error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClientError::Spawn("service stdin was not piped".into()))?;
        let mut writer = BufWriter::new(stdin);
        let initialization = ProtocolFrame::Initialize {
            protocol_version: PROTOCOL_VERSION,
            legacy_storage: bootstrap.legacy_storage,
        };
        let bytes = encode_frame(&initialization).map_err(ClientError::Protocol)?;
        if let Err(error) = writer.write_all(&bytes).and_then(|()| writer.flush()) {
            drop(writer);
            let status = match child.try_wait() {
                Ok(Some(status)) => Some(status),
                _ => {
                    let _ = child.kill();
                    child.wait().ok()
                }
            };
            return match status {
                Some(status) => Err(ClientError::ServiceExited(status.code())),
                None => Err(ClientError::Io(error.to_string())),
            };
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ClientError::Spawn("service stdout was not piped".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ClientError::Spawn("service stderr was not piped".into()))?;
        thread::Builder::new()
            .name("studio-service-stderr".into())
            .spawn(move || drain_stderr(stderr))
            .map_err(|error| ClientError::Spawn(error.to_string()))?;

        let (sender, incoming) = mpsc::sync_channel(64);
        thread::Builder::new()
            .name("studio-service-protocol".into())
            .spawn(move || read_protocol(stdout, sender))
            .map_err(|error| ClientError::Spawn(error.to_string()))?;

        let mut client = Self {
            child,
            writer: Some(writer),
            incoming,
            next_request: 1,
            latest_snapshot: empty_snapshot(),
        };
        client.await_ready()?;
        Ok(client)
    }

    pub fn latest_snapshot(&self) -> &StudioSnapshot {
        &self.latest_snapshot
    }

    pub fn send(
        &mut self,
        action: StudioAction,
        expected_revision: Option<StudioRevision>,
    ) -> Result<RequestId, ClientError> {
        let request_id = RequestId(format!("studio-{}", self.next_request));
        self.next_request = self.next_request.saturating_add(1);
        let frame = ProtocolFrame::Request(StudioRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.clone(),
            expected_revision,
            action,
        });
        self.write_frame(&frame)?;
        Ok(request_id)
    }

    pub fn try_recv(&mut self) -> Result<Option<ProtocolFrame>, ClientError> {
        match self.incoming.try_recv() {
            Ok(frame) => self.accept_frame(frame).map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                let status = self
                    .child
                    .try_wait()
                    .ok()
                    .flatten()
                    .and_then(|status| status.code());
                Err(ClientError::ServiceExited(status))
            }
        }
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<ProtocolFrame, ClientError> {
        match self.incoming.recv_timeout(timeout) {
            Ok(frame) => self.accept_frame(frame),
            Err(RecvTimeoutError::Timeout) => Err(ClientError::Timeout("waiting for a response")),
            Err(RecvTimeoutError::Disconnected) => {
                let status = self
                    .child
                    .try_wait()
                    .ok()
                    .flatten()
                    .and_then(|status| status.code());
                Err(ClientError::ServiceExited(status))
            }
        }
    }

    pub fn shutdown(mut self, timeout: Duration) -> Result<(), ClientError> {
        self.writer.take();
        let deadline = Instant::now() + timeout.min(SHUTDOWN_TIMEOUT);
        loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| ClientError::Io(error.to_string()))?
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err(ClientError::ServiceExited(status.code()))
                };
            }
            if Instant::now() >= deadline {
                self.child
                    .kill()
                    .map_err(|error| ClientError::Io(error.to_string()))?;
                let _ = self.child.wait();
                return Err(ClientError::Timeout("service shutdown"));
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn await_ready(&mut self) -> Result<(), ClientError> {
        let deadline = Instant::now() + START_TIMEOUT;
        let mut ready = false;
        let mut snapshot = None;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ClientError::Timeout("service startup"));
            }
            let frame = self.recv_timeout(remaining)?;
            match frame {
                ProtocolFrame::Event(StudioEvent::ServiceReady { protocol_version }) => {
                    if protocol_version != PROTOCOL_VERSION {
                        return Err(ClientError::Protocol(format!(
                            "service protocol {protocol_version} does not match client protocol {PROTOCOL_VERSION}"
                        )));
                    }
                    ready = true;
                }
                ProtocolFrame::Event(StudioEvent::Snapshot(value)) => snapshot = Some(value),
                ProtocolFrame::Event(StudioEvent::ProtocolError(error)) => {
                    return Err(ClientError::Protocol(error.message));
                }
                _ => {}
            }
            if ready && let Some(snapshot) = snapshot {
                self.latest_snapshot = *snapshot;
                return Ok(());
            }
        }
    }

    fn accept_frame(
        &mut self,
        frame: Result<ProtocolFrame, ClientError>,
    ) -> Result<ProtocolFrame, ClientError> {
        let frame = frame?;
        if let ProtocolFrame::Event(StudioEvent::Snapshot(snapshot)) = &frame {
            self.latest_snapshot = snapshot.as_ref().clone();
        }
        if let ProtocolFrame::Response(response) = &frame {
            self.latest_snapshot = response.snapshot.clone();
        }
        Ok(frame)
    }

    fn write_frame(&mut self, frame: &ProtocolFrame) -> Result<(), ClientError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or(ClientError::ServiceExited(None))?;
        let bytes = encode_frame(frame).map_err(ClientError::Protocol)?;
        writer
            .write_all(&bytes)
            .and_then(|()| writer.flush())
            .map_err(|error| ClientError::Io(error.to_string()))
    }
}

impl Drop for StudioClient {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

pub fn service_sibling_path(gui_executable: &Path) -> PathBuf {
    service_sibling_path_for(gui_executable, cfg!(windows))
}

fn service_sibling_path_for(gui_executable: &Path, windows: bool) -> PathBuf {
    let mut filename: OsString = "vapor-studio-service".into();
    if windows {
        filename.push(".exe");
    }
    gui_executable
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}

fn read_protocol(
    stdout: impl std::io::Read,
    sender: SyncSender<Result<ProtocolFrame, ClientError>>,
) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_frame(&mut reader) {
            Ok(Some(frame)) => {
                if sender.send(Ok(frame)).is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(error) => {
                let _ = sender.send(Err(ClientError::Protocol(error)));
                return;
            }
        }
    }
}

fn drain_stderr(stderr: impl std::io::Read) {
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|size| size > 0) {
        eprintln!("[vapor-studio-service] {}", line.trim_end());
        line.clear();
    }
}

fn empty_snapshot() -> StudioSnapshot {
    use vapor_studio_protocol::{HostSummary, StudioRevision};
    StudioSnapshot {
        revision: StudioRevision(0),
        host: HostSummary {
            app_root: None,
            workspace: None,
            mode: "starting".into(),
            platform: String::new(),
            steam_client_available: false,
        },
        active_profile_id: None,
        durable_state: vapor_studio_protocol::DurableStateSnapshot::default(),
        active_operation: None,
        catalog: Vec::new(),
        workspace_content: vapor_studio_protocol::WorkspaceContentSnapshot::default(),
        selected_content_id: None,
        available_actions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientError, ServiceBootstrap, StudioClient, service_sibling_path_for};
    use std::path::Path;

    fn bootstrap() -> ServiceBootstrap {
        ServiceBootstrap {
            application_executable: Path::new("/app/bin/vapor-gui").to_path_buf(),
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
            legacy_storage: vapor_studio_protocol::LegacyStorageSeed::default(),
            state_directory: None,
        }
    }

    #[test]
    fn service_is_resolved_as_gui_sibling_on_each_target() {
        let linux = service_sibling_path_for(Path::new("/app/bin/vapor-gui"), false);
        assert_eq!(linux.file_name().unwrap(), "vapor-studio-service");
        assert_eq!(linux.parent().unwrap(), Path::new("/app/bin"));
        let windows = service_sibling_path_for(Path::new("C:/app/bin/vapor-gui.exe"), true);
        assert_eq!(windows.file_name().unwrap(), "vapor-studio-service.exe");
        assert_eq!(windows.parent().unwrap(), Path::new("C:/app/bin"));
    }

    #[test]
    fn missing_sibling_is_reported_as_a_spawn_error() {
        let missing = std::env::temp_dir().join(format!(
            "vapor-studio-service-missing-{}",
            std::process::id()
        ));
        assert!(matches!(
            StudioClient::spawn(&missing, bootstrap()),
            Err(ClientError::Spawn(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn early_exit_and_malformed_output_are_bounded_startup_errors() {
        use std::{fs, os::unix::fs::PermissionsExt, time::SystemTime};

        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock follows epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "vapor-studio-client-process-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temporary test directory");
        let exiting = root.join("exit-service");
        fs::write(&exiting, "#!/bin/sh\nexit 0\n").expect("write exiting child");
        fs::set_permissions(&exiting, fs::Permissions::from_mode(0o700))
            .expect("make child executable");
        let exited = StudioClient::spawn(&exiting, bootstrap())
            .err()
            .expect("early exit fails");
        assert!(matches!(exited, ClientError::ServiceExited(_)));

        let malformed = root.join("malformed-service");
        fs::write(&malformed, "#!/bin/sh\nprintf '{broken json}\\n'\n")
            .expect("write malformed child");
        fs::set_permissions(&malformed, fs::Permissions::from_mode(0o700))
            .expect("make child executable");
        let error = StudioClient::spawn(&malformed, bootstrap())
            .err()
            .expect("malformed service output fails startup");
        assert!(
            matches!(error, ClientError::Protocol(message) if message.contains("malformed protocol data"))
        );
        fs::remove_dir_all(root).expect("clean temporary test directory");
    }
}
