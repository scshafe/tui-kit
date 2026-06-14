use std::collections::VecDeque;

use anyhow::Result;
use base64::Engine;
use tui_kit::input::{InputEvent, KeyEvent, MouseEvent};
use tui_kit::remote::{
    decode_frame, decode_input_event, encode_input_event, parse_run_spec, remote_ssh_args,
    RemoteExitStatus, RemoteFrameDecoder, RemoteInputWriter, RemoteLineInputWriter, RemoteOutput,
    RemoteRunSpec, RemoteSession, RemoteSessionEvent, RemoteSessionState, RemoteStderrPolicy,
    RemoteTransport, SshRemoteTransport, REMOTE_PROTOCOL_ENV, REMOTE_PROTOCOL_VERSION,
    REMOTE_RENDER_ENV,
};
use tui_kit::renderer::RenderEffect;

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[derive(Debug)]
struct MemoryTransport {
    lines: VecDeque<String>,
    try_wait_status: Option<RemoteExitStatus>,
    wait_status: RemoteExitStatus,
    stderr_policy: RemoteStderrPolicy,
}

impl MemoryTransport {
    fn new(lines: Vec<String>) -> Self {
        Self {
            lines: lines.into(),
            try_wait_status: Some(RemoteExitStatus::success()),
            wait_status: RemoteExitStatus::success(),
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }

    fn eof_before_exit(lines: Vec<String>, wait_status: RemoteExitStatus) -> Self {
        Self {
            lines: lines.into(),
            try_wait_status: None,
            wait_status,
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }

    fn child_exits(status: RemoteExitStatus) -> Self {
        Self {
            lines: VecDeque::new(),
            try_wait_status: Some(status),
            wait_status: status,
            stderr_policy: RemoteStderrPolicy::Null,
        }
    }
}

impl RemoteTransport for MemoryTransport {
    fn read_output_line(&mut self) -> Result<Option<String>> {
        Ok(self.lines.pop_front())
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
fn parses_run_spec_and_builds_ssh_args() {
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
fn input_event_codec_round_trips_typed_events() {
    for event in [
        InputEvent::Key(KeyEvent::Char('λ')),
        InputEvent::Key(KeyEvent::CtrlC),
        InputEvent::Mouse(MouseEvent::Drag { x: 7, y: 9 }),
        InputEvent::Mouse(MouseEvent::Release),
        InputEvent::Resize {
            cols: 120,
            rows: 40,
        },
    ] {
        let encoded = encode_input_event(event);
        assert_eq!(decode_input_event(&encoded).unwrap(), event);
    }
}

#[test]
fn line_input_writer_forwards_typed_input_as_protocol_lines() {
    let mut writer = RemoteLineInputWriter::new(Vec::new());

    writer
        .write_input_event(InputEvent::Resize { cols: 80, rows: 24 })
        .unwrap();
    writer
        .write_input_event(InputEvent::Key(KeyEvent::Enter))
        .unwrap();

    let output = String::from_utf8(writer.into_inner()).unwrap();
    assert_eq!(output, "resize 80 24\nkey enter\n");
}

#[test]
fn decodes_remote_frame_text_and_effects() {
    let text = base64::engine::general_purpose::STANDARD.encode("hello");
    let png = base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3]);
    let lines = args(&[
        "TKF1 20 2",
        &format!("TEXT 0 {text}"),
        &format!("EFFECT ensure_image_loaded 7 {png}"),
        "EFFECT place_image 7 70 1 2 3 4 5 6 7 8",
        "EFFECT delete_placement 70",
        "END",
    ]);

    let frame = decode_frame(&lines).unwrap();

    let rendered = frame
        .buffer()
        .content()
        .iter()
        .take(5)
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(rendered, "hello");
    assert_eq!(frame.effects().len(), 3);
    assert!(matches!(
        frame.effects()[0],
        RenderEffect::EnsureImageLoaded { image_id: 7, .. }
    ));
    assert!(matches!(
        frame.effects()[1],
        RenderEffect::PlaceImage { origin, options }
            if origin.col == 1
                && origin.row == 2
                && options.image_id == 7
                && options.placement_id == 70
    ));
    assert_eq!(
        frame.effects()[2],
        RenderEffect::DeletePlacement { placement_id: 70 }
    );
}

#[test]
fn streaming_decoder_separates_passthrough_and_complete_frames() {
    let text = base64::engine::general_purpose::STANDARD.encode("ok");
    let mut decoder = RemoteFrameDecoder::new();

    assert_eq!(
        decoder.push_line("plain output").unwrap(),
        Some(RemoteOutput::Passthrough("plain output".to_owned()))
    );
    assert_eq!(decoder.push_line("TKF1 4 1").unwrap(), None);
    assert!(decoder.is_pending());
    assert_eq!(decoder.push_line(format!("TEXT 0 {text}")).unwrap(), None);
    let output = decoder.push_line("END").unwrap().unwrap();

    let RemoteOutput::Frame(frame) = output else {
        panic!("expected frame output");
    };
    assert_eq!(frame.area().width, 4);
    assert!(!decoder.is_pending());
}

#[test]
fn remote_session_reports_passthrough_then_first_frame_then_child_exit() {
    let text = base64::engine::general_purpose::STANDARD.encode("ok");
    let mut session = RemoteSession::new(MemoryTransport::new(args(&[
        "remote banner",
        "TKF1 4 1",
        &format!("TEXT 0 {text}"),
        "END",
    ])));

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
fn remote_session_errors_when_eof_arrives_mid_frame() {
    let mut session = RemoteSession::new(MemoryTransport::eof_before_exit(
        args(&["TKF1 4 1"]),
        RemoteExitStatus::success(),
    ));

    let error = session.next_event().unwrap_err().to_string();

    assert!(error.contains("remote stream ended before completing a frame"));
}

#[test]
fn remote_session_surfaces_malformed_completed_frame() {
    let mut session = RemoteSession::new(MemoryTransport::new(args(&["TKF1 bad 1", "END"])));

    assert!(session.next_event().is_err());
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
