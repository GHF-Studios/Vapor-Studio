use crate::{
    ServiceBootstrap, StudioSession, host::StudioHostContext, persistence::StudioStateStore,
};
use std::{
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    sync::mpsc::{self, RecvTimeoutError, SyncSender},
    thread,
    time::Duration,
};
use vapor_studio_protocol::{
    LegacyStorageSeed, MAX_PROTOCOL_FRAME_BYTES, PROTOCOL_VERSION, ProtocolFrame, StudioError,
    StudioErrorCode, StudioEvent,
};

const MAX_FRAME_BYTES: usize = MAX_PROTOCOL_FRAME_BYTES;

pub fn run_stdio_service(
    bootstrap: ServiceBootstrap,
    host_context: StudioHostContext,
) -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BufReader::new(stdin);
    let seed = read_initialization(&mut input)?;
    let session = if bootstrap.fixture_mode {
        StudioSession::new(bootstrap)
    } else {
        StudioSession::with_persistent_state(
            bootstrap,
            host_context.clone(),
            StudioStateStore::new(host_context.state_directory.clone()),
            Some(&seed),
        )?
    };
    serve(input, BufWriter::new(stdout.lock()), session)
}

fn read_initialization(input: &mut impl BufRead) -> Result<LegacyStorageSeed, String> {
    let mut buffer = Vec::new();
    let read = input
        .by_ref()
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut buffer)
        .map_err(|error| format!("failed reading service initialization: {error}"))?;
    if read == 0 {
        return Err("client closed before service initialization".to_owned());
    }
    if buffer.len() > MAX_FRAME_BYTES {
        return Err("service initialization exceeds the 32 MiB limit".to_owned());
    }
    if buffer.last() == Some(&b'\n') {
        buffer.pop();
    }
    if buffer.last() == Some(&b'\r') {
        buffer.pop();
    }
    let frame: ProtocolFrame = serde_json::from_slice(&buffer)
        .map_err(|error| format!("invalid service initialization: {error}"))?;
    match frame {
        ProtocolFrame::Initialize {
            protocol_version,
            legacy_storage,
        } if protocol_version == PROTOCOL_VERSION => Ok(legacy_storage),
        ProtocolFrame::Initialize {
            protocol_version, ..
        } => Err(format!(
            "client protocol {protocol_version} is unsupported; service protocol is {PROTOCOL_VERSION}"
        )),
        _ => Err("expected a service initialization frame".to_owned()),
    }
}

fn serve(
    input: impl BufRead + Send + 'static,
    mut output: impl Write,
    mut session: StudioSession,
) -> Result<(), String> {
    write_frame(
        &mut output,
        &ProtocolFrame::Event(StudioEvent::ServiceReady {
            protocol_version: PROTOCOL_VERSION,
        }),
    )?;
    write_frame(
        &mut output,
        &ProtocolFrame::Event(StudioEvent::Snapshot(Box::new(session.snapshot()))),
    )?;

    let (sender, receiver) = mpsc::sync_channel(8);
    thread::Builder::new()
        .name("studio-service-protocol-input".to_owned())
        .spawn(move || read_requests(input, sender))
        .map_err(|error| format!("could not start service input reader: {error}"))?;

    loop {
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(Some(mut buffer))) => {
                if buffer.last() == Some(&b'\n') {
                    buffer.pop();
                }
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
                if buffer.is_empty() {
                    continue;
                }
                process_frame(&buffer, &mut output, &mut session)?;
            }
            Ok(Ok(None)) => return Ok(()),
            Ok(Err(message)) => {
                let limit_exceeded = message == "protocol frame limit exceeded";
                write_frame(
                    &mut output,
                    &ProtocolFrame::Event(StudioEvent::ProtocolError(StudioError {
                        code: StudioErrorCode::InvalidRequest,
                        message: if limit_exceeded {
                            "protocol frame exceeds the 32 MiB limit".to_owned()
                        } else {
                            message.clone()
                        },
                        retryable: false,
                    })),
                )?;
                return Err(message);
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => {}
        }

        write_background_updates(&mut output, &mut session)?;
    }
}

fn read_requests(input: impl BufRead, sender: SyncSender<Result<Option<Vec<u8>>, String>>) {
    let mut input = input;
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = match input
            .by_ref()
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut buffer)
        {
            Ok(read) => read,
            Err(error) => {
                let _ = sender.send(Err(format!("failed reading protocol input: {error}")));
                return;
            }
        };
        if read == 0 {
            let _ = sender.send(Ok(None));
            return;
        }
        if buffer.len() > MAX_FRAME_BYTES {
            let _ = sender.send(Err("protocol frame limit exceeded".to_owned()));
            return;
        }
        if sender.send(Ok(Some(buffer.clone()))).is_err() {
            return;
        }
    }
}

fn process_frame(
    buffer: &[u8],
    output: &mut impl Write,
    session: &mut StudioSession,
) -> Result<(), String> {
    match serde_json::from_slice::<ProtocolFrame>(buffer) {
        Ok(ProtocolFrame::Initialize { .. }) => {
            let error = StudioError {
                code: StudioErrorCode::InvalidRequest,
                message: "service initialization is only valid as the first frame".to_owned(),
                retryable: false,
            };
            write_frame(
                output,
                &ProtocolFrame::Event(StudioEvent::ProtocolError(error)),
            )?;
        }
        Ok(ProtocolFrame::Request(request)) => {
            let response = if request.protocol_version != PROTOCOL_VERSION {
                vapor_studio_protocol::StudioResponse {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: request.request_id,
                    revision: session.snapshot().revision,
                    kind: vapor_studio_protocol::StudioResponseKind::Rejected,
                    snapshot: session.snapshot(),
                    error: Some(StudioError {
                        code: StudioErrorCode::UnsupportedProtocol,
                        message: format!(
                            "client protocol {} is unsupported; service protocol is {}",
                            request.protocol_version, PROTOCOL_VERSION
                        ),
                        retryable: false,
                    }),
                }
            } else {
                session.dispatch(request)
            };
            write_frame(output, &ProtocolFrame::Response(Box::new(response)))?;
        }
        Ok(_) => {
            write_frame(
                output,
                &ProtocolFrame::Event(StudioEvent::ProtocolError(StudioError {
                    code: StudioErrorCode::InvalidRequest,
                    message: "service accepts request frames only".to_owned(),
                    retryable: false,
                })),
            )?;
        }
        Err(error) => {
            write_frame(
                output,
                &ProtocolFrame::Event(StudioEvent::ProtocolError(StudioError {
                    code: StudioErrorCode::InvalidRequest,
                    message: format!("invalid protocol frame: {error}"),
                    retryable: false,
                })),
            )?;
        }
    }
    write_background_updates(output, session)?;
    Ok(())
}

fn write_background_updates(
    output: &mut impl Write,
    session: &mut StudioSession,
) -> Result<(), String> {
    let events = session.poll_background_operations();
    if events.is_empty() {
        return Ok(());
    }
    for event in events {
        write_frame(output, &ProtocolFrame::Event(event))?;
    }
    write_frame(
        output,
        &ProtocolFrame::Event(StudioEvent::Snapshot(Box::new(session.snapshot()))),
    )
}

fn write_frame(writer: &mut impl Write, frame: &ProtocolFrame) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(frame)
        .map_err(|error| format!("failed encoding protocol response: {error}"))?;
    if bytes.len() + 1 > MAX_FRAME_BYTES {
        return Err(format!(
            "service protocol response exceeds the {MAX_FRAME_BYTES} byte limit"
        ));
    }
    bytes.push(b'\n');
    writer
        .write_all(&bytes)
        .and_then(|()| writer.flush())
        .map_err(|error| format!("failed writing protocol response: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, path::PathBuf};
    use vapor_studio_protocol::{CatalogAction, ProtocolFrame, StudioAction, StudioRequest};

    fn fixture_session() -> StudioSession {
        StudioSession::new(ServiceBootstrap {
            application_executable: PathBuf::from("/app/bin/vapor-gui"),
            workshop_app_id: None,
            workspace_root: None,
            active_profile_id: Some("default".to_owned()),
            fixture_mode: true,
        })
    }

    fn frames(output: &[u8]) -> Vec<ProtocolFrame> {
        output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("server emits valid JSON frames"))
            .collect()
    }

    #[test]
    fn server_handshakes_and_correlates_requests() {
        let request = ProtocolFrame::Request(StudioRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: vapor_studio_protocol::RequestId("test-1".into()),
            expected_revision: None,
            action: StudioAction::Catalog(CatalogAction::Refresh),
        });
        let input = format!("{}\n", serde_json::to_string(&request).unwrap());
        let mut output = Vec::new();
        serve(Cursor::new(input), &mut output, fixture_session()).unwrap();
        let frames = frames(&output);
        assert!(matches!(
            frames[0],
            ProtocolFrame::Event(StudioEvent::ServiceReady { .. })
        ));
        assert!(matches!(
            frames[1],
            ProtocolFrame::Event(StudioEvent::Snapshot(_))
        ));
        let ProtocolFrame::Response(response) = &frames[2] else {
            panic!("expected correlated response")
        };
        assert_eq!(response.request_id.0, "test-1");
        assert_eq!(response.revision.0, 1);
    }

    #[test]
    fn unsupported_versions_are_rejected_and_malformed_frames_do_not_kill_the_session() {
        let unsupported = ProtocolFrame::Request(StudioRequest {
            protocol_version: PROTOCOL_VERSION + 1,
            request_id: vapor_studio_protocol::RequestId("future-client".into()),
            expected_revision: None,
            action: StudioAction::Catalog(CatalogAction::Refresh),
        });
        let valid = ProtocolFrame::Request(StudioRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: vapor_studio_protocol::RequestId("current-client".into()),
            expected_revision: None,
            action: StudioAction::Catalog(CatalogAction::Refresh),
        });
        let input = format!(
            "{}\nnot-json\n{}\n",
            serde_json::to_string(&unsupported).unwrap(),
            serde_json::to_string(&valid).unwrap()
        );
        let mut output = Vec::new();
        serve(Cursor::new(input), &mut output, fixture_session()).unwrap();
        let frames = frames(&output);
        assert!(matches!(
            &frames[2],
            ProtocolFrame::Response(response)
                if response.error.as_ref().is_some_and(|error| error.code == StudioErrorCode::UnsupportedProtocol)
        ));
        assert!(matches!(
            &frames[3],
            ProtocolFrame::Event(StudioEvent::ProtocolError(error))
                if error.code == StudioErrorCode::InvalidRequest
        ));
        assert!(matches!(
            &frames[4],
            ProtocolFrame::Response(response) if response.revision.0 == 1
        ));
    }

    #[test]
    fn oversized_input_is_rejected_with_a_bounded_protocol_error() {
        let mut input = vec![b'x'; MAX_FRAME_BYTES + 1];
        input.push(b'\n');
        let mut output = Vec::new();
        assert!(serve(Cursor::new(input), &mut output, fixture_session()).is_err());
        assert!(matches!(
            frames(&output).last(),
            Some(ProtocolFrame::Event(StudioEvent::ProtocolError(error)))
                if error.message.contains("32 MiB limit")
        ));
    }
}
