use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::Result;
use tui_kit::image::PlaceOptions;
use tui_kit::input::{InputEvent, KeyEvent, MouseEvent};
use tui_kit::layout::{CellOffset, PixelRect};
use tui_kit::remote::{
    decode_client_message, decode_server_message, encode_client_message, encode_server_message,
    parse_run_spec, remote_ssh_args, RemoteClientMessage, RemoteExitStatus, RemoteFrameMessage,
    RemoteInputMessage, RemoteInputWriter, RemoteKeyMessage, RemoteMouseMessage,
    RemoteProtocolDecoder, RemoteProtocolInputWriter, RemoteProtocolOutput,
    RemoteRenderEffectMessage, RemoteRunSpec, RemoteServerMessage, RemoteSession,
    RemoteSessionEvent, RemoteSessionState, RemoteStderrPolicy, RemoteTextRow, RemoteTransport,
    SshRemoteTransport, REMOTE_MAX_FRAME_BYTES, REMOTE_PROTOCOL_ENV, REMOTE_PROTOCOL_NAME,
    REMOTE_PROTOCOL_VERSION, REMOTE_RENDER_ENV,
};
use tui_kit::renderer::RenderEffect;

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn raw_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = format!("TKF2 {}\n", payload.len()).into_bytes();
    frame.extend_from_slice(payload);
    frame
}

fn server_hello() -> RemoteServerMessage {
    RemoteServerMessage::Hello {
        protocol: REMOTE_PROTOCOL_NAME.to_owned(),
        version: REMOTE_PROTOCOL_VERSION.to_owned(),
    }
}

fn text_frame_message() -> RemoteServerMessage {
    RemoteServerMessage::Frame {
        frame: RemoteFrameMessage {
            cols: 4,
            rows: 1,
            text_rows: vec![RemoteTextRow {
                row: 0,
                text: "ok".to_owned(),
            }],
            effects: Vec::new(),
        },
    }
}

fn encode_messages(messages: &[RemoteServerMessage]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for message in messages {
        bytes.extend(encode_server_message(message).unwrap());
    }
    bytes
}

#[derive(Debug)]
struct MemoryTransport {
    output: VecDeque<u8>,
    try_wait_status: Option<RemoteExitStatus>,
    wait_status: RemoteExitStatus,
    stderr_policy: RemoteStderrPolicy,
}

impl MemoryTransport {
    fn new(output: Vec<u8>) -> Self {
        Self {
            output: output.into(),
            try_wait_status: Some(RemoteExitStatus::success()),
            wait_status: RemoteExitStatus::success(),
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }

    fn eof_before_exit(output: Vec<u8>, wait_status: RemoteExitStatus) -> Self {
        Self {
            output: output.into(),
            try_wait_status: None,
            wait_status,
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }

    fn child_exits(status: RemoteExitStatus) -> Self {
        Self {
            output: VecDeque::new(),
            try_wait_status: Some(status),
            wait_status: status,
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }
}

impl RemoteTransport for MemoryTransport {
    fn read_output(&mut self, buf: &mut [u8]) -> Result<usize> {
        let count = buf.len().min(self.output.len());
        for slot in buf.iter_mut().take(count) {
            *slot = self.output.pop_front().expect("count checked above");
        }
        Ok(count)
    }

    fn try_wait(&mut self) -> Result<Option<RemoteExitStatus>> {
        Ok(self.try_wait_status.take())
    }

    fn wait(&mut self) -> Result<RemoteExitStatus> {
        Ok(self.wait_status)
    }

    fn stderr_policy(&self) -> RemoteStderrPolicy {
        self.stderr_policy
    }
}

#[test]
fn parses_run_spec_and_builds_v2_ssh_args() {
    let spec: RemoteRunSpec = parse_run_spec(&args(&[
        "run",
        "ssh://alice@example.com",
        "--",
        "remote-app",
        "--flag",
    ]))
    .unwrap();

    assert_eq!(spec.host, "alice@example.com");
    assert_eq!(spec.command, args(&["remote-app", "--flag"]));
    assert_eq!(REMOTE_PROTOCOL_VERSION, "tui-kit-remote-v2");
    assert_eq!(
        remote_ssh_args(&spec),
        args(&[
            "-tt",
            "alice@example.com",
            "env",
            &format!("{REMOTE_RENDER_ENV}=1"),
            &format!("{REMOTE_PROTOCOL_ENV}={REMOTE_PROTOCOL_VERSION}"),
            "remote-app",
            "--flag",
        ])
    );
}

#[test]
fn rejects_missing_run_separator_or_command() {
    assert!(parse_run_spec(&args(&["run", "ssh://host", "app"])).is_err());
    assert!(parse_run_spec(&args(&["run", "ssh://host", "--"])).is_err());
    assert!(parse_run_spec(&args(&["run", "host", "--", "app"])).is_err());
}

#[test]
fn protocol_frame_encoder_decoder_round_trips_hello() {
    let message = server_hello();
    let encoded = encode_server_message(&message).unwrap();

    assert_eq!(decode_server_message(&encoded).unwrap(), message);

    let mut decoder = RemoteProtocolDecoder::new();
    assert_eq!(
        decoder.push_bytes(&encoded).unwrap(),
        vec![RemoteProtocolOutput::Message(message)]
    );
    assert!(!decoder.is_pending());
}

#[test]
fn protocol_frame_encoder_decoder_round_trips_text_and_all_effects() {
    let place_options = PlaceOptions {
        image_id: 7,
        placement_id: 70,
        source: PixelRect {
            x: 3,
            y: 4,
            width: 30,
            height: 40,
        },
        cell_cols: 5,
        cell_rows: 6,
    };
    let frame = RemoteFrameMessage {
        cols: 20,
        rows: 2,
        text_rows: vec![RemoteTextRow {
            row: 0,
            text: "hello".to_owned(),
        }],
        effects: vec![
            RemoteRenderEffectMessage::EnsureImageLoaded {
                image_id: 7,
                png_base64: "AQID".to_owned(),
            },
            RemoteRenderEffectMessage::PlaceImage {
                origin: CellOffset { col: 1, row: 2 },
                options: place_options,
            },
            RemoteRenderEffectMessage::DeleteImagePlacement {
                image_id: 7,
                placement_id: 70,
            },
            RemoteRenderEffectMessage::DeletePlacement { placement_id: 70 },
            RemoteRenderEffectMessage::DeleteAllPlacements,
            RemoteRenderEffectMessage::ForgetAllImages,
            RemoteRenderEffectMessage::FlushImages,
        ],
    };
    let message = RemoteServerMessage::Frame {
        frame: frame.clone(),
    };

    assert_eq!(
        decode_server_message(&encode_server_message(&message).unwrap()).unwrap(),
        message
    );

    let render_frame = frame.to_render_frame().unwrap();
    let rendered = render_frame
        .buffer()
        .content()
        .iter()
        .take(5)
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(rendered, "hello");
    assert_eq!(
        render_frame.effects(),
        &[
            RenderEffect::EnsureImageLoaded {
                image_id: 7,
                png: Arc::from([1u8, 2, 3].as_slice()),
            },
            RenderEffect::PlaceImage {
                origin: CellOffset { col: 1, row: 2 },
                options: place_options,
            },
            RenderEffect::DeleteImagePlacement {
                image_id: 7,
                placement_id: 70,
            },
            RenderEffect::DeletePlacement { placement_id: 70 },
            RenderEffect::DeleteAllPlacements,
            RenderEffect::ForgetAllImages,
            RenderEffect::FlushImages,
        ]
    );
    assert_eq!(RemoteFrameMessage::from_render_frame(&render_frame), frame);
}

#[test]
fn protocol_frame_encoder_decoder_round_trips_all_input_event_variants() {
    let events = [
        InputEvent::Key(KeyEvent::Char('λ')),
        InputEvent::Key(KeyEvent::Up),
        InputEvent::Key(KeyEvent::Down),
        InputEvent::Key(KeyEvent::Left),
        InputEvent::Key(KeyEvent::Right),
        InputEvent::Key(KeyEvent::Enter),
        InputEvent::Key(KeyEvent::Tab),
        InputEvent::Key(KeyEvent::Back),
        InputEvent::Key(KeyEvent::Esc),
        InputEvent::Key(KeyEvent::CtrlC),
        InputEvent::Key(KeyEvent::Unknown),
        InputEvent::Mouse(MouseEvent::Click { x: 1, y: 2 }),
        InputEvent::Mouse(MouseEvent::Drag { x: 3, y: 4 }),
        InputEvent::Mouse(MouseEvent::WheelUp { x: 5, y: 6 }),
        InputEvent::Mouse(MouseEvent::WheelDown { x: 7, y: 8 }),
        InputEvent::Mouse(MouseEvent::Release),
        InputEvent::Resize {
            cols: 120,
            rows: 40,
        },
    ];

    for event in events {
        let message = RemoteClientMessage::Input {
            event: event.into(),
        };
        let decoded = decode_client_message(&encode_client_message(&message).unwrap()).unwrap();
        assert_eq!(decoded, message);
        let RemoteClientMessage::Input { event: wire_event } = decoded else {
            panic!("expected input message");
        };
        assert_eq!(InputEvent::from(wire_event), event);
    }

    assert_eq!(
        RemoteInputMessage::Key {
            key: RemoteKeyMessage::Enter
        },
        InputEvent::Key(KeyEvent::Enter).into()
    );
    assert_eq!(
        RemoteInputMessage::Mouse {
            mouse: RemoteMouseMessage::Release
        },
        InputEvent::Mouse(MouseEvent::Release).into()
    );
}

#[test]
fn protocol_input_writer_frames_typed_input_messages() {
    let mut writer = RemoteProtocolInputWriter::new(Vec::new());

    writer
        .write_input_event(InputEvent::Resize { cols: 80, rows: 24 })
        .unwrap();

    assert_eq!(
        decode_client_message(&writer.into_inner()).unwrap(),
        RemoteClientMessage::Input {
            event: RemoteInputMessage::Resize { cols: 80, rows: 24 }
        }
    );
}

#[test]
fn decoder_reports_passthrough_before_first_header() {
    let mut bytes = b"plain output\n".to_vec();
    bytes.extend(encode_server_message(&server_hello()).unwrap());
    let mut decoder = RemoteProtocolDecoder::new();

    assert_eq!(
        decoder.push_bytes(&bytes).unwrap(),
        vec![
            RemoteProtocolOutput::Passthrough("plain output".to_owned()),
            RemoteProtocolOutput::Message(server_hello()),
        ]
    );
}

#[test]
fn decoder_rejects_invalid_header_length() {
    let mut decoder = RemoteProtocolDecoder::new();

    let error = decoder
        .push_bytes(b"TKF2 nope\n{}")
        .unwrap_err()
        .to_string();

    assert!(error.contains("invalid remote payload length"));
}

#[test]
fn decoder_rejects_payloads_above_maximum() {
    let mut decoder = RemoteProtocolDecoder::new();
    let header = format!("TKF2 {}\n", REMOTE_MAX_FRAME_BYTES + 1);

    let error = decoder
        .push_bytes(header.as_bytes())
        .unwrap_err()
        .to_string();

    assert!(error.contains("exceeds maximum"));
}

#[test]
fn decoder_rejects_eof_mid_payload() {
    let mut decoder = RemoteProtocolDecoder::new();

    assert!(decoder.push_bytes(b"TKF2 10\n{}").unwrap().is_empty());
    let error = decoder.finish().unwrap_err().to_string();

    assert!(error.contains("before completing a protocol payload"));
}

#[test]
fn decoder_rejects_invalid_utf8_json() {
    let mut decoder = RemoteProtocolDecoder::new();

    let error = decoder.push_bytes(b"TKF2 1\n\xff").unwrap_err().to_string();

    assert!(error.contains("not utf-8"));
}

#[test]
fn decoder_rejects_unknown_or_malformed_message_variants() {
    let unknown = raw_frame(br#"{"type":"bogus"}"#);
    let malformed = raw_frame(br#"{"type":"hello","protocol":"tui-kit-remote"}"#);

    assert!(decode_server_message(&unknown).is_err());
    assert!(decode_server_message(&malformed).is_err());
}

#[test]
fn remote_session_reports_passthrough_consumes_hello_then_first_frame_and_child_exit() {
    let mut bytes = b"remote banner\n".to_vec();
    bytes.extend(encode_messages(&[server_hello(), text_frame_message()]));
    let mut session = RemoteSession::new(MemoryTransport::new(bytes));

    assert_eq!(session.state(), RemoteSessionState::Startup);
    assert_eq!(
        session.transport().stderr_policy(),
        RemoteStderrPolicy::Null
    );
    assert_eq!(
        session.next_event().unwrap(),
        RemoteSessionEvent::Passthrough("remote banner".to_owned())
    );
    assert_eq!(session.state(), RemoteSessionState::Passthrough);

    let RemoteSessionEvent::Frame(frame) = session.next_event().unwrap() else {
        panic!("expected first frame");
    };
    assert_eq!(frame.area().width, 4);
    assert_eq!(session.state(), RemoteSessionState::Streaming);

    assert_eq!(
        session.next_event().unwrap(),
        RemoteSessionEvent::ChildExited(RemoteExitStatus::success())
    );
    assert_eq!(session.state(), RemoteSessionState::ChildExited);
}

#[test]
fn remote_session_errors_on_frame_before_hello() {
    let mut session = RemoteSession::new(MemoryTransport::new(encode_messages(&[
        text_frame_message(),
    ])));

    let error = session.next_event().unwrap_err().to_string();

    assert!(error.contains("before protocol hello"));
}

#[test]
fn remote_session_errors_on_version_mismatch() {
    let mut session = RemoteSession::new(MemoryTransport::new(encode_messages(&[
        RemoteServerMessage::Hello {
            protocol: REMOTE_PROTOCOL_NAME.to_owned(),
            version: "tui-kit-remote-v1".to_owned(),
        },
    ])));

    let error = session.next_event().unwrap_err().to_string();

    assert!(error.contains("version mismatch"));
}

#[test]
fn remote_session_surfaces_remote_error_messages() {
    let mut session = RemoteSession::new(MemoryTransport::new(encode_messages(&[
        RemoteServerMessage::Error {
            message: "remote app failed".to_owned(),
        },
    ])));

    let error = session.next_event().unwrap_err().to_string();

    assert!(error.contains("remote app failed"));
}

#[test]
fn remote_session_errors_when_eof_arrives_mid_payload() {
    let mut session = RemoteSession::new(MemoryTransport::eof_before_exit(
        b"TKF2 10\n{}".to_vec(),
        RemoteExitStatus::success(),
    ));

    let error = session.next_event().unwrap_err().to_string();

    assert!(error.contains("before completing a protocol payload"));
}

#[test]
fn remote_session_reports_eof_before_wait_when_child_status_is_not_ready() {
    let mut session = RemoteSession::new(MemoryTransport::eof_before_exit(
        Vec::new(),
        RemoteExitStatus::success(),
    ));

    assert_eq!(session.next_event().unwrap(), RemoteSessionEvent::Eof);
    assert_eq!(session.state(), RemoteSessionState::Eof);
    assert_eq!(
        session.wait_for_exit().unwrap(),
        RemoteExitStatus::success()
    );
    assert_eq!(session.state(), RemoteSessionState::ChildExited);
}

#[test]
fn remote_session_reports_non_zero_child_exit() {
    let mut session =
        RemoteSession::new(MemoryTransport::child_exits(RemoteExitStatus::failure(23)));

    assert_eq!(
        session.next_event().unwrap(),
        RemoteSessionEvent::ChildExited(RemoteExitStatus::failure(23))
    );
    assert!(!RemoteExitStatus::failure(23).is_success());
    assert_eq!(RemoteExitStatus::failure(23).code(), Some(23));
}

#[test]
fn ssh_transport_type_is_part_of_the_public_remote_boundary() {
    let _ = std::mem::size_of::<SshRemoteTransport>();
}
