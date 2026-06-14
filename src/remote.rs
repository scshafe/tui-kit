//! Prototype remote-render protocol helpers for `tui-kit-cli`.
//!
//! This module is intentionally explicit. It defines the handshake environment
//! variables, the line-oriented input codec, the `TKF1` frame decoder, and the
//! transport/session boundary used by the prototype launcher. It is not a
//! stable wire protocol.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::image::PlaceOptions;
use crate::input::{InputEvent, KeyEvent, MouseEvent};
use crate::layout::{CellOffset, PixelRect};
use crate::renderer::{RenderEffect, RenderFrame};

pub const REMOTE_RENDER_ENV: &str = "TUI_KIT_REMOTE_RENDER";
pub const REMOTE_PROTOCOL_ENV: &str = "TUI_KIT_REMOTE_PROTOCOL";
pub const REMOTE_PROTOCOL_VERSION: &str = "tui-kit-remote-v1";
pub const CLI_USAGE: &str = "usage: tui-kit-cli run ssh://host -- app [args...]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRunSpec {
    pub host: String,
    pub command: Vec<String>,
}

impl RemoteRunSpec {
    pub fn new(host: impl Into<String>, command: impl IntoIterator<Item = String>) -> Result<Self> {
        let host = host.into();
        if host.is_empty() {
            bail!("remote host cannot be empty");
        }
        let command = command.into_iter().collect::<Vec<_>>();
        if command.is_empty() {
            bail!("remote command cannot be empty");
        }
        Ok(Self { host, command })
    }
}

pub fn parse_run_spec(args: &[String]) -> Result<RemoteRunSpec> {
    if args.first().map(String::as_str) != Some("run") {
        bail!("{CLI_USAGE}");
    }
    let target = args.get(1).ok_or_else(|| anyhow!(CLI_USAGE))?;
    let host = target
        .strip_prefix("ssh://")
        .ok_or_else(|| anyhow!("remote target must start with ssh://"))?;
    if args.get(2).map(String::as_str) != Some("--") {
        bail!("missing `--` before remote command\n{CLI_USAGE}");
    }
    RemoteRunSpec::new(host, args[3..].iter().cloned())
}

pub fn remote_ssh_args(spec: &RemoteRunSpec) -> Vec<String> {
    let mut args = vec![
        "-tt".to_owned(),
        spec.host.clone(),
        "env".to_owned(),
        format!("{REMOTE_RENDER_ENV}=1"),
        format!("{REMOTE_PROTOCOL_ENV}={REMOTE_PROTOCOL_VERSION}"),
    ];
    args.extend(spec.command.iter().cloned());
    args
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteStderrPolicy {
    Inherit,
    Null,
}

impl RemoteStderrPolicy {
    fn stdio(self) -> Stdio {
        match self {
            Self::Inherit => Stdio::inherit(),
            Self::Null => Stdio::null(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteExitStatus {
    success: bool,
    code: Option<i32>,
}

impl RemoteExitStatus {
    pub fn success() -> Self {
        Self {
            success: true,
            code: Some(0),
        }
    }

    pub fn failure(code: i32) -> Self {
        Self {
            success: false,
            code: Some(code),
        }
    }

    pub fn from_exit_status(status: ExitStatus) -> Self {
        Self {
            success: status.success(),
            code: status.code(),
        }
    }

    pub fn is_success(self) -> bool {
        self.success
    }

    pub fn code(self) -> Option<i32> {
        self.code
    }
}

impl fmt::Display for RemoteExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.success, self.code) {
            (true, Some(code)) => write!(f, "exit code {code}"),
            (true, None) => f.write_str("successful exit"),
            (false, Some(code)) => write!(f, "exit code {code}"),
            (false, None) => f.write_str("terminated without an exit code"),
        }
    }
}

pub trait RemoteInputWriter {
    fn write_input_event(&mut self, input: InputEvent) -> Result<()>;
}

#[derive(Debug)]
pub struct RemoteLineInputWriter<W> {
    writer: W,
}

impl<W> RemoteLineInputWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> RemoteInputWriter for RemoteLineInputWriter<W> {
    fn write_input_event(&mut self, input: InputEvent) -> Result<()> {
        writeln!(self.writer, "{}", encode_input_event(input))
            .context("failed to write remote input event")?;
        self.writer
            .flush()
            .context("failed to flush remote input event")?;
        Ok(())
    }
}

pub trait RemoteTransport {
    fn read_output_line(&mut self) -> Result<Option<String>>;
    fn try_wait(&mut self) -> Result<Option<RemoteExitStatus>>;
    fn wait(&mut self) -> Result<RemoteExitStatus>;
    fn stderr_policy(&self) -> RemoteStderrPolicy;
}

#[derive(Debug)]
pub struct SshRemoteTransport {
    child: Child,
    stdout: BufReader<std::process::ChildStdout>,
    stderr_policy: RemoteStderrPolicy,
}

impl SshRemoteTransport {
    pub fn spawn(
        spec: &RemoteRunSpec,
        stderr_policy: RemoteStderrPolicy,
    ) -> Result<(Self, impl RemoteInputWriter + Send + 'static)> {
        let ssh_args = remote_ssh_args(spec);
        let mut child = Command::new("ssh")
            .args(&ssh_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr_policy.stdio())
            .spawn()
            .with_context(|| format!("failed to spawn ssh for {}", spec.host))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture ssh stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture ssh stdout"))?;
        Ok((
            Self {
                child,
                stdout: BufReader::new(stdout),
                stderr_policy,
            },
            RemoteLineInputWriter::new(stdin),
        ))
    }
}

impl RemoteTransport for SshRemoteTransport {
    fn read_output_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .context("failed to read remote stdout")?;
        if bytes == 0 {
            return Ok(None);
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Ok(Some(line))
    }

    fn try_wait(&mut self) -> Result<Option<RemoteExitStatus>> {
        self.child
            .try_wait()
            .context("failed polling ssh child")
            .map(|status| status.map(RemoteExitStatus::from_exit_status))
    }

    fn wait(&mut self) -> Result<RemoteExitStatus> {
        self.child
            .wait()
            .context("failed waiting for ssh child")
            .map(RemoteExitStatus::from_exit_status)
    }

    fn stderr_policy(&self) -> RemoteStderrPolicy {
        self.stderr_policy
    }
}

pub fn encode_input_event(input: InputEvent) -> String {
    match input {
        InputEvent::Key(key) => encode_key_event(key),
        InputEvent::Mouse(mouse) => encode_mouse_event(mouse),
        InputEvent::Resize { cols, rows } => format!("resize {cols} {rows}"),
    }
}

pub fn decode_input_event(line: &str) -> Result<InputEvent> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["key", "char", codepoint] => {
            let value = parse_u32(codepoint, "char codepoint")?;
            let Some(ch) = char::from_u32(value) else {
                bail!("invalid char codepoint: {value}");
            };
            Ok(InputEvent::Key(KeyEvent::Char(ch)))
        }
        ["key", name] => Ok(InputEvent::Key(decode_named_key(name)?)),
        ["mouse", "click", x, y] => Ok(InputEvent::Mouse(MouseEvent::Click {
            x: parse_u16(x, "mouse x")?,
            y: parse_u16(y, "mouse y")?,
        })),
        ["mouse", "drag", x, y] => Ok(InputEvent::Mouse(MouseEvent::Drag {
            x: parse_u16(x, "mouse x")?,
            y: parse_u16(y, "mouse y")?,
        })),
        ["mouse", "wheel-up", x, y] => Ok(InputEvent::Mouse(MouseEvent::WheelUp {
            x: parse_u16(x, "mouse x")?,
            y: parse_u16(y, "mouse y")?,
        })),
        ["mouse", "wheel-down", x, y] => Ok(InputEvent::Mouse(MouseEvent::WheelDown {
            x: parse_u16(x, "mouse x")?,
            y: parse_u16(y, "mouse y")?,
        })),
        ["mouse", "release"] => Ok(InputEvent::Mouse(MouseEvent::Release)),
        ["resize", cols, rows] => Ok(InputEvent::Resize {
            cols: parse_u16(cols, "resize cols")?,
            rows: parse_u16(rows, "resize rows")?,
        }),
        _ => bail!("invalid input event line: {line}"),
    }
}

fn encode_key_event(key: KeyEvent) -> String {
    match key {
        KeyEvent::Char(ch) => format!("key char {}", u32::from(ch)),
        KeyEvent::Up => "key up".to_owned(),
        KeyEvent::Down => "key down".to_owned(),
        KeyEvent::Left => "key left".to_owned(),
        KeyEvent::Right => "key right".to_owned(),
        KeyEvent::Enter => "key enter".to_owned(),
        KeyEvent::Tab => "key tab".to_owned(),
        KeyEvent::Back => "key back".to_owned(),
        KeyEvent::Esc => "key esc".to_owned(),
        KeyEvent::CtrlC => "key ctrl-c".to_owned(),
        KeyEvent::Unknown => "key unknown".to_owned(),
    }
}

fn decode_named_key(name: &str) -> Result<KeyEvent> {
    Ok(match name {
        "up" => KeyEvent::Up,
        "down" => KeyEvent::Down,
        "left" => KeyEvent::Left,
        "right" => KeyEvent::Right,
        "enter" => KeyEvent::Enter,
        "tab" => KeyEvent::Tab,
        "back" => KeyEvent::Back,
        "esc" => KeyEvent::Esc,
        "ctrl-c" => KeyEvent::CtrlC,
        "unknown" => KeyEvent::Unknown,
        _ => bail!("invalid key name: {name}"),
    })
}

fn encode_mouse_event(mouse: MouseEvent) -> String {
    match mouse {
        MouseEvent::Click { x, y } => format!("mouse click {x} {y}"),
        MouseEvent::Drag { x, y } => format!("mouse drag {x} {y}"),
        MouseEvent::WheelUp { x, y } => format!("mouse wheel-up {x} {y}"),
        MouseEvent::WheelDown { x, y } => format!("mouse wheel-down {x} {y}"),
        MouseEvent::Release => "mouse release".to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteOutput {
    Frame(RenderFrame),
    Passthrough(String),
}

#[derive(Debug, Default)]
pub struct RemoteFrameDecoder {
    pending: Vec<String>,
}

impl RemoteFrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn push_line(&mut self, line: impl Into<String>) -> Result<Option<RemoteOutput>> {
        let line = line.into();
        if self.pending.is_empty() {
            if line.starts_with("TKF1 ") {
                self.pending.push(line);
                return Ok(None);
            }
            return Ok(Some(RemoteOutput::Passthrough(line)));
        }

        self.pending.push(line);
        if self.pending.last().map(String::as_str) == Some("END") {
            let lines = self.pending.iter().map(String::as_str).collect::<Vec<_>>();
            let result = decode_frame(&lines).map(RemoteOutput::Frame);
            self.pending.clear();
            return result.map(Some);
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSessionState {
    Startup,
    AwaitingFirstFrame,
    Streaming,
    Passthrough,
    Eof,
    ChildExited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteSessionEvent {
    Frame(RenderFrame),
    Passthrough(String),
    Eof,
    ChildExited(RemoteExitStatus),
}

#[derive(Debug)]
pub struct RemoteSession<T> {
    transport: T,
    decoder: RemoteFrameDecoder,
    state: RemoteSessionState,
    saw_frame: bool,
}

impl<T: RemoteTransport> RemoteSession<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            decoder: RemoteFrameDecoder::new(),
            state: RemoteSessionState::Startup,
            saw_frame: false,
        }
    }

    pub fn state(&self) -> RemoteSessionState {
        self.state
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn next_event(&mut self) -> Result<RemoteSessionEvent> {
        if !self.saw_frame
            && matches!(
                self.state,
                RemoteSessionState::Startup | RemoteSessionState::Passthrough
            )
        {
            self.state = RemoteSessionState::AwaitingFirstFrame;
        }

        loop {
            let Some(line) = self.transport.read_output_line()? else {
                if self.decoder.is_pending() {
                    bail!("remote stream ended before completing a frame");
                }
                if let Some(status) = self.transport.try_wait()? {
                    self.state = RemoteSessionState::ChildExited;
                    return Ok(RemoteSessionEvent::ChildExited(status));
                }
                self.state = RemoteSessionState::Eof;
                return Ok(RemoteSessionEvent::Eof);
            };

            match self.decoder.push_line(line)? {
                Some(RemoteOutput::Frame(frame)) => {
                    self.saw_frame = true;
                    self.state = RemoteSessionState::Streaming;
                    return Ok(RemoteSessionEvent::Frame(frame));
                }
                Some(RemoteOutput::Passthrough(line)) => {
                    self.state = RemoteSessionState::Passthrough;
                    return Ok(RemoteSessionEvent::Passthrough(line));
                }
                None => {}
            }
        }
    }

    pub fn wait_for_exit(&mut self) -> Result<RemoteExitStatus> {
        if self.decoder.is_pending() {
            bail!("remote stream ended before completing a frame");
        }
        let status = self.transport.wait()?;
        self.state = RemoteSessionState::ChildExited;
        Ok(status)
    }
}

pub fn decode_frame(lines: &[impl AsRef<str>]) -> Result<RenderFrame> {
    let first = lines
        .first()
        .map(AsRef::as_ref)
        .ok_or_else(|| anyhow!("remote frame is empty"))?;
    let header = first.split_whitespace().collect::<Vec<_>>();
    let ["TKF1", cols, rows] = header.as_slice() else {
        bail!("invalid remote frame header: {first}");
    };
    let cols = parse_u16(cols, "frame cols")?;
    let rows = parse_u16(rows, "frame rows")?;
    let mut buffer = Buffer::empty(Rect::new(0, 0, cols, rows));
    let mut effects = Vec::new();
    let mut ended = false;

    for raw in lines.iter().skip(1).map(AsRef::as_ref) {
        if raw == "END" {
            ended = true;
            break;
        }
        if let Some(rest) = raw.strip_prefix("TEXT ") {
            decode_text_line(rest, rows, &mut buffer)?;
            continue;
        }
        if let Some(rest) = raw.strip_prefix("EFFECT ") {
            effects.push(decode_effect(rest)?);
            continue;
        }
        bail!("invalid remote frame line: {raw}");
    }

    if !ended {
        bail!("remote frame missing END line");
    }

    Ok(RenderFrame::new(buffer, effects))
}

fn decode_text_line(rest: &str, rows: u16, buffer: &mut Buffer) -> Result<()> {
    let mut parts = rest.splitn(2, ' ');
    let row = parts
        .next()
        .ok_or_else(|| anyhow!("TEXT line missing row"))?;
    let text = parts
        .next()
        .ok_or_else(|| anyhow!("TEXT line missing base64 text"))?;
    let row = parse_u16(row, "text row")?;
    if row >= rows {
        bail!("TEXT row {row} is outside frame height {rows}");
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(text)
        .context("invalid TEXT base64")?;
    let text = String::from_utf8(decoded).context("TEXT payload is not utf-8")?;
    buffer.set_string(0, row, text, Style::default());
    Ok(())
}

fn decode_effect(rest: &str) -> Result<RenderEffect> {
    let parts = rest.split_whitespace().collect::<Vec<_>>();
    Ok(match parts.as_slice() {
        ["ensure_image_loaded", image_id, png] => {
            let png = base64::engine::general_purpose::STANDARD
                .decode(png)
                .context("invalid image payload base64")?;
            RenderEffect::EnsureImageLoaded {
                image_id: parse_u32(image_id, "image_id")?,
                png: Arc::from(png.into_boxed_slice()),
            }
        }
        ["place_image", image_id, placement_id, origin_col, origin_row, source_x, source_y, source_width, source_height, cell_cols, cell_rows] => {
            RenderEffect::PlaceImage {
                origin: CellOffset {
                    col: parse_u16(origin_col, "origin_col")?,
                    row: parse_u16(origin_row, "origin_row")?,
                },
                options: PlaceOptions {
                    image_id: parse_u32(image_id, "image_id")?,
                    placement_id: parse_u32(placement_id, "placement_id")?,
                    source: PixelRect {
                        x: parse_u32(source_x, "source_x")?,
                        y: parse_u32(source_y, "source_y")?,
                        width: parse_u32(source_width, "source_width")?,
                        height: parse_u32(source_height, "source_height")?,
                    },
                    cell_cols: parse_u16(cell_cols, "cell_cols")?,
                    cell_rows: parse_u16(cell_rows, "cell_rows")?,
                },
            }
        }
        ["delete_image_placement", image_id, placement_id] => RenderEffect::DeleteImagePlacement {
            image_id: parse_u32(image_id, "image_id")?,
            placement_id: parse_u32(placement_id, "placement_id")?,
        },
        ["delete_placement", placement_id] => RenderEffect::DeletePlacement {
            placement_id: parse_u32(placement_id, "placement_id")?,
        },
        ["delete_all_placements"] => RenderEffect::DeleteAllPlacements,
        ["forget_all_images"] => RenderEffect::ForgetAllImages,
        ["flush_images"] => RenderEffect::FlushImages,
        _ => bail!("invalid EFFECT line: {rest}"),
    })
}

fn parse_u16(value: &str, name: &str) -> Result<u16> {
    value
        .parse::<u16>()
        .with_context(|| format!("invalid {name}"))
}

fn parse_u32(value: &str, name: &str) -> Result<u32> {
    value
        .parse::<u32>()
        .with_context(|| format!("invalid {name}"))
}
