use crate::{ServiceBootstrap, StudioSession};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use vapor_studio_protocol::{
    PROTOCOL_VERSION, ProtocolFrame, StudioError, StudioErrorCode, StudioEvent,
};

const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub fn run_stdio_service(bootstrap: ServiceBootstrap) -> Result<(), String> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(
        BufReader::new(stdin.lock()),
        BufWriter::new(stdout.lock()),
        StudioSession::new(bootstrap),
    )
}

fn serve(
    mut input: impl BufRead,
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

    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = input
            .by_ref()
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut buffer)
            .map_err(|error| format!("failed reading protocol input: {error}"))?;
        if read == 0 {
            return Ok(());
        }
        if buffer.len() > MAX_FRAME_BYTES {
            let error = StudioError {
                code: StudioErrorCode::InvalidRequest,
                message: "protocol frame exceeds the 1 MiB limit".to_owned(),
                retryable: false,
            };
            write_frame(
                &mut output,
                &ProtocolFrame::Event(StudioEvent::ProtocolError(error)),
            )?;
            return Err("protocol frame limit exceeded".to_owned());
        }
        if buffer.last() == Some(&b'\n') {
            buffer.pop();
        }
        if buffer.last() == Some(&b'\r') {
            buffer.pop();
        }
        if buffer.is_empty() {
            continue;
        }
        match serde_json::from_slice::<ProtocolFrame>(&buffer) {
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
                write_frame(&mut output, &ProtocolFrame::Response(Box::new(response)))?;
            }
            Ok(_) => {
                write_frame(
                    &mut output,
                    &ProtocolFrame::Event(StudioEvent::ProtocolError(StudioError {
                        code: StudioErrorCode::InvalidRequest,
                        message: "service accepts request frames only".to_owned(),
                        retryable: false,
                    })),
                )?;
            }
            Err(error) => {
                write_frame(
                    &mut output,
                    &ProtocolFrame::Event(StudioEvent::ProtocolError(StudioError {
                        code: StudioErrorCode::InvalidRequest,
                        message: format!("invalid protocol frame: {error}"),
                        retryable: false,
                    })),
                )?;
            }
        }
    }
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
                if error.message.contains("1 MiB limit")
        ));
    }
}
