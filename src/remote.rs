//! Typed remote-render protocol helpers for `tui-kit-cli`.
//!
//! This module defines the remote launcher environment variables, typed JSON
//! wire messages, length-prefixed `TKF2` framing, input forwarding, and the
//! transport/session boundary used by the SSH launcher. The protocol is
//! versioned and intentionally strict: a server must send a valid `Hello`
//! before frame messages are accepted.

use std::collections::VecDeque;
use std::fmt;
use std::io::{Read, Write};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::image::PlaceOptions;
use crate::input::{InputEvent, KeyEvent, MouseEvent};
use crate::layout::CellOffset;
use crate::renderer::{RenderEffect, RenderFrame};

pub const REMOTE_RENDER_ENV: &str = "TUI_KIT_REMOTE_RENDER";
pub const REMOTE_PROTOCOL_ENV: &str = "TUI_KIT_REMOTE_PROTOCOL";
pub const REMOTE_PROTOCOL_NAME: &str = "tui-kit-remote";
pub const REMOTE_PROTOCOL_VERSION: &str = "tui-kit-remote-v2";
pub const REMOTE_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const CLI_USAGE: &str = "usage: tui-kit-cli run ssh://host -- app [args...]";

const FRAME_HEADER_PREFIX: &[u8] = b"TKF2 ";
const FRAME_HEADER_MAGIC: &str = "TKF2";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteServerMessage {
    Hello { protocol: String, version: String },
    Frame { frame: RemoteFrameMessage },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteClientMessage {
    Hello { protocol: String, version: String },
    Input { event: RemoteInputMessage },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFrameMessage {
    pub cols: u16,
    pub rows: u16,
    pub text_rows: Vec<RemoteTextRow>,
    pub effects: Vec<RemoteRenderEffectMessage>,
}

impl RemoteFrameMessage {
    pub fn from_render_frame(frame: &RenderFrame) -> Self {
        let area = frame.area();
        let mut text_rows = Vec::new();
        for row in 0..area.height {
            let mut text = String::new();
            for col in 0..area.width {
                let x = area.x.saturating_add(col);
                let y = area.y.saturating_add(row);
                if let Some(cell) = frame.buffer().cell((x, y)) {
                    text.push_str(cell.symbol());
                }
            }
            let text = text.trim_end_matches(' ').to_owned();
            if !text.is_empty() {
                text_rows.push(RemoteTextRow { row, text });
            }
        }
        let effects = frame
            .effects()
            .iter()
            .map(RemoteRenderEffectMessage::from)
            .collect();
        Self {
            cols: area.width,
            rows: area.height,
            text_rows,
            effects,
        }
    }

    pub fn to_render_frame(&self) -> Result<RenderFrame> {
        let mut buffer = Buffer::empty(Rect::new(0, 0, self.cols, self.rows));
        for row in &self.text_rows {
            if row.row >= self.rows {
                bail!(
                    "remote text row {} is outside frame height {}",
                    row.row,
                    self.rows
                );
            }
            buffer.set_string(0, row.row, &row.text, Style::default());
        }
        let effects = self
            .effects
            .iter()
            .cloned()
            .map(RenderEffect::try_from)
            .collect::<Result<Vec<_>>>()?;
        Ok(RenderFrame::new(buffer, effects))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTextRow {
    pub row: u16,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteRenderEffectMessage {
    EnsureImageLoaded {
        image_id: u32,
        png_base64: String,
    },
    PlaceImage {
        origin: CellOffset,
        options: PlaceOptions,
    },
    DeleteImagePlacement {
        image_id: u32,
        placement_id: u32,
    },
    DeletePlacement {
        placement_id: u32,
    },
    DeleteAllPlacements,
    ForgetAllImages,
    FlushImages,
}

impl From<&RenderEffect> for RemoteRenderEffectMessage {
    fn from(effect: &RenderEffect) -> Self {
        match effect {
            RenderEffect::EnsureImageLoaded { image_id, png } => Self::EnsureImageLoaded {
                image_id: *image_id,
                png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            },
            RenderEffect::PlaceImage { origin, options } => Self::PlaceImage {
                origin: *origin,
                options: *options,
            },
            RenderEffect::DeleteImagePlacement {
                image_id,
                placement_id,
            } => Self::DeleteImagePlacement {
                image_id: *image_id,
                placement_id: *placement_id,
            },
            RenderEffect::DeletePlacement { placement_id } => Self::DeletePlacement {
                placement_id: *placement_id,
            },
            RenderEffect::DeleteAllPlacements => Self::DeleteAllPlacements,
            RenderEffect::ForgetAllImages => Self::ForgetAllImages,
            RenderEffect::FlushImages => Self::FlushImages,
        }
    }
}

impl TryFrom<RemoteRenderEffectMessage> for RenderEffect {
    type Error = anyhow::Error;

    fn try_from(effect: RemoteRenderEffectMessage) -> Result<Self> {
        Ok(match effect {
            RemoteRenderEffectMessage::EnsureImageLoaded {
                image_id,
                png_base64,
            } => {
                let png = base64::engine::general_purpose::STANDARD
                    .decode(png_base64)
                    .context("invalid remote image payload base64")?;
                Self::EnsureImageLoaded {
                    image_id,
                    png: Arc::from(png.into_boxed_slice()),
                }
            }
            RemoteRenderEffectMessage::PlaceImage { origin, options } => {
                Self::PlaceImage { origin, options }
            }
            RemoteRenderEffectMessage::DeleteImagePlacement {
                image_id,
                placement_id,
            } => Self::DeleteImagePlacement {
                image_id,
                placement_id,
            },
            RemoteRenderEffectMessage::DeletePlacement { placement_id } => {
                Self::DeletePlacement { placement_id }
            }
            RemoteRenderEffectMessage::DeleteAllPlacements => Self::DeleteAllPlacements,
            RemoteRenderEffectMessage::ForgetAllImages => Self::ForgetAllImages,
            RemoteRenderEffectMessage::FlushImages => Self::FlushImages,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteInputMessage {
    Key { key: RemoteKeyMessage },
    Mouse { mouse: RemoteMouseMessage },
    Resize { cols: u16, rows: u16 },
}

impl From<InputEvent> for RemoteInputMessage {
    fn from(input: InputEvent) -> Self {
        match input {
            InputEvent::Key(key) => Self::Key { key: key.into() },
            InputEvent::Mouse(mouse) => Self::Mouse {
                mouse: mouse.into(),
            },
            InputEvent::Resize { cols, rows } => Self::Resize { cols, rows },
        }
    }
}

impl From<RemoteInputMessage> for InputEvent {
    fn from(input: RemoteInputMessage) -> Self {
        match input {
            RemoteInputMessage::Key { key } => Self::Key(key.into()),
            RemoteInputMessage::Mouse { mouse } => Self::Mouse(mouse.into()),
            RemoteInputMessage::Resize { cols, rows } => Self::Resize { cols, rows },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteKeyMessage {
    Char { ch: char },
    Up,
    Down,
    Left,
    Right,
    Enter,
    Tab,
    Back,
    Esc,
    CtrlC,
    Unknown,
}

impl From<KeyEvent> for RemoteKeyMessage {
    fn from(key: KeyEvent) -> Self {
        match key {
            KeyEvent::Char(ch) => Self::Char { ch },
            KeyEvent::Up => Self::Up,
            KeyEvent::Down => Self::Down,
            KeyEvent::Left => Self::Left,
            KeyEvent::Right => Self::Right,
            KeyEvent::Enter => Self::Enter,
            KeyEvent::Tab => Self::Tab,
            KeyEvent::Back => Self::Back,
            KeyEvent::Esc => Self::Esc,
            KeyEvent::CtrlC => Self::CtrlC,
            KeyEvent::Unknown => Self::Unknown,
        }
    }
}

impl From<RemoteKeyMessage> for KeyEvent {
    fn from(key: RemoteKeyMessage) -> Self {
        match key {
            RemoteKeyMessage::Char { ch } => Self::Char(ch),
            RemoteKeyMessage::Up => Self::Up,
            RemoteKeyMessage::Down => Self::Down,
            RemoteKeyMessage::Left => Self::Left,
            RemoteKeyMessage::Right => Self::Right,
            RemoteKeyMessage::Enter => Self::Enter,
            RemoteKeyMessage::Tab => Self::Tab,
            RemoteKeyMessage::Back => Self::Back,
            RemoteKeyMessage::Esc => Self::Esc,
            RemoteKeyMessage::CtrlC => Self::CtrlC,
            RemoteKeyMessage::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteMouseMessage {
    Click { x: u16, y: u16 },
    Drag { x: u16, y: u16 },
    WheelUp { x: u16, y: u16 },
    WheelDown { x: u16, y: u16 },
    Release,
}

impl From<MouseEvent> for RemoteMouseMessage {
    fn from(mouse: MouseEvent) -> Self {
        match mouse {
            MouseEvent::Click { x, y } => Self::Click { x, y },
            MouseEvent::Drag { x, y } => Self::Drag { x, y },
            MouseEvent::WheelUp { x, y } => Self::WheelUp { x, y },
            MouseEvent::WheelDown { x, y } => Self::WheelDown { x, y },
            MouseEvent::Release => Self::Release,
        }
    }
}

impl From<RemoteMouseMessage> for MouseEvent {
    fn from(mouse: RemoteMouseMessage) -> Self {
        match mouse {
            RemoteMouseMessage::Click { x, y } => Self::Click { x, y },
            RemoteMouseMessage::Drag { x, y } => Self::Drag { x, y },
            RemoteMouseMessage::WheelUp { x, y } => Self::WheelUp { x, y },
            RemoteMouseMessage::WheelDown { x, y } => Self::WheelDown { x, y },
            RemoteMouseMessage::Release => Self::Release,
        }
    }
}

pub fn encode_server_message(message: &RemoteServerMessage) -> Result<Vec<u8>> {
    encode_protocol_message(message)
}

pub fn decode_server_message(frame: &[u8]) -> Result<RemoteServerMessage> {
    decode_protocol_message(frame)
}

pub fn encode_client_message(message: &RemoteClientMessage) -> Result<Vec<u8>> {
    encode_protocol_message(message)
}

pub fn decode_client_message(frame: &[u8]) -> Result<RemoteClientMessage> {
    decode_protocol_message(frame)
}

fn encode_protocol_message<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    let payload =
        serde_json::to_vec(message).context("failed to encode remote protocol message")?;
    if payload.len() > REMOTE_MAX_FRAME_BYTES {
        bail!(
            "remote payload length {} exceeds maximum {}",
            payload.len(),
            REMOTE_MAX_FRAME_BYTES
        );
    }
    let mut frame = format!("{FRAME_HEADER_MAGIC} {}\n", payload.len()).into_bytes();
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_protocol_message<T: DeserializeOwned>(frame: &[u8]) -> Result<T> {
    let newline = frame
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| anyhow!("remote protocol frame missing header terminator"))?;
    let length = parse_header_length(&frame[..newline])?;
    let payload_start = newline + 1;
    let payload_end = payload_start
        .checked_add(length)
        .ok_or_else(|| anyhow!("remote payload length overflow"))?;
    if frame.len() < payload_end {
        bail!("remote protocol frame ended before payload was complete");
    }
    if frame.len() > payload_end {
        bail!("remote protocol frame contains trailing bytes");
    }
    decode_payload(&frame[payload_start..payload_end])
}

fn decode_payload<T: DeserializeOwned>(payload: &[u8]) -> Result<T> {
    let json = std::str::from_utf8(payload).context("remote protocol payload is not utf-8")?;
    serde_json::from_str(json).context("malformed remote protocol message")
}

fn parse_header_length(header: &[u8]) -> Result<usize> {
    let header = std::str::from_utf8(header).context("remote protocol header is not utf-8")?;
    let mut parts = header.split_whitespace();
    let Some(magic) = parts.next() else {
        bail!("remote protocol header is empty");
    };
    if magic != FRAME_HEADER_MAGIC {
        bail!("invalid remote protocol header: {header}");
    }
    let length = parts
        .next()
        .ok_or_else(|| anyhow!("remote protocol header missing payload length"))?;
    if parts.next().is_some() {
        bail!("invalid remote protocol header: {header}");
    }
    let length = length
        .parse::<usize>()
        .context("invalid remote payload length")?;
    if length > REMOTE_MAX_FRAME_BYTES {
        bail!("remote payload length {length} exceeds maximum {REMOTE_MAX_FRAME_BYTES}");
    }
    Ok(length)
}

pub trait RemoteInputWriter {
    fn write_input_event(&mut self, input: InputEvent) -> Result<()>;
}

#[derive(Debug)]
pub struct RemoteProtocolInputWriter<W> {
    writer: W,
}

impl<W> RemoteProtocolInputWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> RemoteProtocolInputWriter<W> {
    pub fn write_client_message(&mut self, message: &RemoteClientMessage) -> Result<()> {
        let frame = encode_client_message(message)?;
        self.writer
            .write_all(&frame)
            .context("failed to write remote protocol message")?;
        self.writer
            .flush()
            .context("failed to flush remote protocol message")?;
        Ok(())
    }
}

impl<W: Write> RemoteInputWriter for RemoteProtocolInputWriter<W> {
    fn write_input_event(&mut self, input: InputEvent) -> Result<()> {
        self.write_client_message(&RemoteClientMessage::Input {
            event: input.into(),
        })
    }
}

pub trait RemoteTransport {
    fn read_output(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn try_wait(&mut self) -> Result<Option<RemoteExitStatus>>;
    fn wait(&mut self) -> Result<RemoteExitStatus>;
    fn stderr_policy(&self) -> RemoteStderrPolicy;
}

#[derive(Debug)]
pub struct SshRemoteTransport {
    child: Child,
    stdout: std::process::ChildStdout,
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
                stdout,
                stderr_policy,
            },
            RemoteProtocolInputWriter::new(stdin),
        ))
    }
}

impl RemoteTransport for SshRemoteTransport {
    fn read_output(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.stdout
            .read(buf)
            .context("failed to read remote stdout")
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteProtocolOutput {
    Message(RemoteServerMessage),
    Passthrough(String),
}

#[derive(Debug, Default)]
pub struct RemoteProtocolDecoder {
    buffer: Vec<u8>,
}

impl RemoteProtocolDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_pending(&self) -> bool {
        !self.buffer.is_empty()
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<RemoteProtocolOutput>> {
        self.buffer.extend_from_slice(bytes);
        let mut outputs = Vec::new();
        while let Some(output) = self.next_output()? {
            outputs.push(output);
        }
        Ok(outputs)
    }

    pub fn finish(&mut self) -> Result<Option<RemoteProtocolOutput>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        if self.buffer.starts_with(FRAME_HEADER_PREFIX) {
            let Some(header_end) = self.buffer.iter().position(|byte| *byte == b'\n') else {
                bail!("remote stream ended before completing a protocol header");
            };
            let length = parse_header_length(&self.buffer[..header_end])?;
            let payload_start = header_end + 1;
            if self.buffer.len() < payload_start + length {
                bail!("remote stream ended before completing a protocol payload");
            }
            return self.next_output();
        }
        if starts_with_header_magic(&self.buffer) {
            bail!("remote stream ended before completing a protocol header");
        }
        let line = std::mem::take(&mut self.buffer);
        Ok(Some(RemoteProtocolOutput::Passthrough(
            passthrough_from_bytes(line)?,
        )))
    }

    fn next_output(&mut self) -> Result<Option<RemoteProtocolOutput>> {
        if self.buffer.is_empty() {
            return Ok(None);
        }

        if self.buffer.starts_with(FRAME_HEADER_PREFIX) {
            let Some(header_end) = self.buffer.iter().position(|byte| *byte == b'\n') else {
                return Ok(None);
            };
            let length = parse_header_length(&self.buffer[..header_end])?;
            let payload_start = header_end + 1;
            let payload_end = payload_start
                .checked_add(length)
                .ok_or_else(|| anyhow!("remote payload length overflow"))?;
            if self.buffer.len() < payload_end {
                return Ok(None);
            }
            let payload = self.buffer[payload_start..payload_end].to_vec();
            self.buffer.drain(..payload_end);
            return Ok(Some(RemoteProtocolOutput::Message(decode_payload(
                &payload,
            )?)));
        }

        if is_partial_header_prefix(&self.buffer) {
            return Ok(None);
        }

        if starts_with_header_magic(&self.buffer) {
            let Some(header_end) = self.buffer.iter().position(|byte| *byte == b'\n') else {
                return Ok(None);
            };
            let invalid = String::from_utf8_lossy(&self.buffer[..header_end]);
            bail!("invalid remote protocol header: {invalid}");
        }

        let Some(line_end) = self.buffer.iter().position(|byte| *byte == b'\n') else {
            return Ok(None);
        };
        let line = self.buffer.drain(..=line_end).collect::<Vec<_>>();
        Ok(Some(RemoteProtocolOutput::Passthrough(
            passthrough_from_bytes(line)?,
        )))
    }
}

fn is_partial_header_prefix(buffer: &[u8]) -> bool {
    buffer.len() < FRAME_HEADER_PREFIX.len() && FRAME_HEADER_PREFIX.starts_with(buffer)
}

fn starts_with_header_magic(buffer: &[u8]) -> bool {
    buffer.len() >= FRAME_HEADER_MAGIC.len()
        && &buffer[..FRAME_HEADER_MAGIC.len()] == FRAME_HEADER_MAGIC.as_bytes()
}

fn passthrough_from_bytes(mut bytes: Vec<u8>) -> Result<String> {
    if bytes.ends_with(b"\n") {
        bytes.pop();
        if bytes.ends_with(b"\r") {
            bytes.pop();
        }
    }
    String::from_utf8(bytes).context("remote passthrough output is not utf-8")
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
    decoder: RemoteProtocolDecoder,
    pending_outputs: VecDeque<RemoteProtocolOutput>,
    state: RemoteSessionState,
    negotiated: bool,
    saw_frame: bool,
}

impl<T: RemoteTransport> RemoteSession<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            decoder: RemoteProtocolDecoder::new(),
            pending_outputs: VecDeque::new(),
            state: RemoteSessionState::Startup,
            negotiated: false,
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
        loop {
            if let Some(output) = self.pending_outputs.pop_front() {
                if let Some(event) = self.handle_output(output)? {
                    return Ok(event);
                }
                continue;
            }

            let mut bytes = [0u8; 8192];
            let count = self.transport.read_output(&mut bytes)?;
            if count == 0 {
                if let Some(output) = self.decoder.finish()? {
                    if let Some(event) = self.handle_output(output)? {
                        return Ok(event);
                    }
                    continue;
                }
                if let Some(status) = self.transport.try_wait()? {
                    self.state = RemoteSessionState::ChildExited;
                    return Ok(RemoteSessionEvent::ChildExited(status));
                }
                self.state = RemoteSessionState::Eof;
                return Ok(RemoteSessionEvent::Eof);
            }

            self.pending_outputs
                .extend(self.decoder.push_bytes(&bytes[..count])?);
        }
    }

    pub fn wait_for_exit(&mut self) -> Result<RemoteExitStatus> {
        if self.decoder.is_pending() {
            bail!("remote stream ended before completing a protocol message");
        }
        let status = self.transport.wait()?;
        self.state = RemoteSessionState::ChildExited;
        Ok(status)
    }

    fn handle_output(
        &mut self,
        output: RemoteProtocolOutput,
    ) -> Result<Option<RemoteSessionEvent>> {
        match output {
            RemoteProtocolOutput::Passthrough(line) => {
                self.state = RemoteSessionState::Passthrough;
                Ok(Some(RemoteSessionEvent::Passthrough(line)))
            }
            RemoteProtocolOutput::Message(message) => self.handle_message(message),
        }
    }

    fn handle_message(
        &mut self,
        message: RemoteServerMessage,
    ) -> Result<Option<RemoteSessionEvent>> {
        match message {
            RemoteServerMessage::Hello { protocol, version } => {
                self.validate_hello(&protocol, &version)?;
                self.negotiated = true;
                if !self.saw_frame {
                    self.state = RemoteSessionState::AwaitingFirstFrame;
                }
                Ok(None)
            }
            RemoteServerMessage::Frame { frame } => {
                if !self.negotiated {
                    bail!("remote frame received before protocol hello");
                }
                let frame = frame.to_render_frame()?;
                self.saw_frame = true;
                self.state = RemoteSessionState::Streaming;
                Ok(Some(RemoteSessionEvent::Frame(frame)))
            }
            RemoteServerMessage::Error { message } => {
                bail!("remote protocol error: {message}");
            }
        }
    }

    fn validate_hello(&self, protocol: &str, version: &str) -> Result<()> {
        if self.negotiated {
            bail!("remote protocol hello received more than once");
        }
        if protocol != REMOTE_PROTOCOL_NAME {
            bail!("remote protocol mismatch: expected {REMOTE_PROTOCOL_NAME}, got {protocol}");
        }
        if version != REMOTE_PROTOCOL_VERSION {
            bail!(
                "remote protocol version mismatch: expected {REMOTE_PROTOCOL_VERSION}, got {version}"
            );
        }
        Ok(())
    }
}
