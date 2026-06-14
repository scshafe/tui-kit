use base64::Engine;
use tui_kit::input::{InputEvent, KeyEvent, MouseEvent};
use tui_kit::remote::{
    decode_frame, decode_input_event, encode_input_event, parse_run_spec, remote_ssh_args,
    RemoteFrameDecoder, RemoteOutput, RemoteRunSpec, REMOTE_PROTOCOL_ENV, REMOTE_PROTOCOL_VERSION,
    REMOTE_RENDER_ENV,
};
use tui_kit::renderer::RenderEffect;

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
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
