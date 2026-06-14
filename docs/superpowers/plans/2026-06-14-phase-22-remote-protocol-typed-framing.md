# Phase 22 - Remote Protocol Typed Framing

**Status:** PLANNED - do not implement until Phase 21 is committed, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 21 c4tui internal visibility hygiene.

## Goal

Replace the prototype `TKF1` line-oriented remote frame format with typed
versioned protocol messages and stricter stream framing. Keep the Phase 13
transport/session lifecycle, but make protocol negotiation, frame payloads,
input forwarding, malformed payload handling, and EOF behavior explicit before
adding host profiles or reconnect behavior.

This is an intentional protocol break. Do not keep `TKF1` compatibility,
deprecated aliases, migration decoders, or dual-format fallback paths.

This phase must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `tui-kit/src/remote.rs` still describes the protocol as a prototype.
- `REMOTE_PROTOCOL_VERSION` is `tui-kit-remote-v1`.
- `RemoteTransport` reads output one line at a time.
- `RemoteFrameDecoder` recognizes frames starting with `TKF1 ` and ending with
  `END`.
- `decode_frame` parses text rows and effects from ad hoc line tokens.
- `RemoteLineInputWriter` forwards input events as line tokens such as
  `key enter` and `resize 80 24`.
- `RemoteSession` already owns startup, awaiting-first-frame, streaming,
  passthrough, EOF, and child-exit states.
- `tests/remote.rs` covers current argument parsing, input event line round
  trips, full frame decoding, passthrough, first frame, EOF mid-frame,
  malformed frame, and child exit.

## Protocol Choice

Use typed JSON messages with a length-prefixed payload and an ASCII magic
header:

```text
TKF2 <length>\n
<length bytes of UTF-8 JSON>
```

Reasons:

- JSON with serde keeps message definitions inspectable and testable.
- The explicit length avoids `END` sentinels and line-token ambiguity.
- The `TKF2` header lets the session distinguish pre-protocol passthrough text
  from framed protocol payloads before negotiation completes.
- The project rule forbids migration layers, so replacing `TKF1` in one phase
  is simpler than supporting both.

Set:

```rust
pub const REMOTE_PROTOCOL_VERSION: &str = "tui-kit-remote-v2";
```

Use a maximum payload size constant to prevent unbounded allocation. The plan
defaults to 16 MiB unless implementation shows current image payload tests need
a different limit:

```rust
pub const REMOTE_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
```

## Message Model

Add typed wire messages in `src/remote.rs` or split into
`src/remote/protocol.rs` if the file becomes too large.

Planned server-to-client messages:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteServerMessage {
    Hello {
        protocol: String,
        version: String,
    },
    Frame {
        frame: RemoteFrameMessage,
    },
    Error {
        message: String,
    },
}
```

Planned client-to-server messages:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteClientMessage {
    Hello {
        protocol: String,
        version: String,
    },
    Input {
        event: RemoteInputMessage,
    },
}
```

Frame payload:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFrameMessage {
    pub cols: u16,
    pub rows: u16,
    pub text_rows: Vec<RemoteTextRow>,
    pub effects: Vec<RemoteRenderEffectMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTextRow {
    pub row: u16,
    pub text: String,
}
```

The current remote prototype only transmits default-style text rows. Preserve
that behavior in this phase. Styled cell transport can be a later protocol
change with its own plan.

Represent image bytes as base64 strings inside the typed JSON effect message,
then convert to `Arc<[u8]>` for `RenderEffect::EnsureImageLoaded`.

Input payload should mirror current `InputEvent` coverage:

- key char and named keys;
- mouse click, drag, wheel up, wheel down, release;
- resize.

## Negotiation

`RemoteSession` should require a valid `Hello` message before accepting a
`Frame`.

Preserve current launcher behavior:

- passthrough output before the first protocol header is reported as
  `RemoteSessionEvent::Passthrough`;
- local terminal entry is still delayed until the first frame;
- child exit before a frame is still reported without entering the terminal.

New behavior requiring tests:

- a `Frame` before `Hello` is an error;
- a missing or mismatched version in `Hello` is an error;
- a valid `Hello` advances session state to awaiting first frame, but does not
  require the CLI to render anything;
- a remote `Error` message becomes a session error with the remote message;
- EOF while a length-prefixed payload is pending is an error;
- payload length above `REMOTE_MAX_FRAME_BYTES` is an error;
- malformed UTF-8 or malformed JSON is an error.

If a public `RemoteSessionEvent::Negotiated` event would force CLI churn with no
consumer value, keep negotiation internal and expose it only through
`RemoteSessionState`.

## Transport and Codec

Replace line-specific names with typed final names. Do not leave aliases.

Planned replacements:

- `RemoteFrameDecoder` -> `RemoteProtocolDecoder`.
- `RemoteLineInputWriter<W>` -> `RemoteProtocolInputWriter<W>`.
- `RemoteInputWriter::write_input_event` should frame a
  `RemoteClientMessage::Input`.
- `RemoteTransport::read_output_line` should be replaced by a byte-oriented or
  framed-message-oriented read method.

Preferred transport trait:

```rust
pub trait RemoteTransport {
    fn read_output(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn try_wait(&mut self) -> Result<Option<RemoteExitStatus>>;
    fn wait(&mut self) -> Result<RemoteExitStatus>;
    fn stderr_policy(&self) -> RemoteStderrPolicy;
}
```

If the decoder is easier to test with `Read`, a generic
`RemoteProtocolReader<R: Read>` can own the low-level byte parsing while
`RemoteSession` adapts the transport.

`SshRemoteTransport` should wrap child stdout as a byte reader instead of a
`BufReader::read_line` source.

## Tests

Replace current `TKF1` tests with typed framing tests. Keep the behavioral
coverage categories from Phase 13.

Required tests:

- run-spec parsing and SSH args use `REMOTE_PROTOCOL_VERSION` v2;
- protocol frame encoder/decoder round trips `Hello`;
- protocol frame encoder/decoder round trips text rows and all current
  `RenderEffect` variants;
- protocol frame encoder/decoder round trips all current input event variants;
- decoder reports passthrough before first `TKF2` header;
- decoder rejects invalid header length;
- decoder rejects payloads above `REMOTE_MAX_FRAME_BYTES`;
- decoder rejects EOF mid-payload;
- decoder rejects invalid UTF-8 JSON;
- decoder rejects unknown or malformed message variants;
- session reports passthrough, consumes valid hello, then reports first frame;
- session errors on frame-before-hello;
- session errors on version mismatch;
- session reports EOF before wait when child status is not ready and no frame
  is pending;
- session reports non-zero child exit;
- CLI input writer stops and joins as before.

## Public Surface

Because `tui_kit::remote` is public, this phase intentionally changes public
names. That is allowed because the protocol is explicitly prototype-level and
the project rule forbids compatibility layers.

Update:

- module docs in `src/remote.rs`;
- `src/lib.rs` top-level remote description if wording still says
  "prototype";
- `docs/public-surface-retention.txt` for any newly public protocol types;
- `tests/remote.rs` imports and assertions;
- `src/bin/tui-kit-cli.rs` imports and input writer usage.

Run `cargo doc --no-deps` and the public-surface audit.

## Non-Goals

- No host profiles.
- No reconnect/resume.
- No persistent remote agent.
- No styled-cell wire format unless required to preserve current tests.
- No c4tui source changes unless the local tui-kit dependency now requires
  them.

## Implementation Tasks

- [ ] Confirm Phase 21 is implemented, verified, and committed.
- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 22 plan document if it is not already present in the
  implementation branch.
- [ ] Change `REMOTE_PROTOCOL_VERSION` to `tui-kit-remote-v2`.
- [ ] Add typed serde wire-message types for server messages, client messages,
  frames, text rows, render effects, and input events.
- [ ] Add length-prefixed `TKF2 <length>\n<payload>` encode/decode helpers with
  maximum payload validation.
- [ ] Replace `RemoteFrameDecoder` with `RemoteProtocolDecoder`.
- [ ] Replace `RemoteLineInputWriter` with `RemoteProtocolInputWriter`.
- [ ] Replace line-oriented input encoding/decoding with typed client messages.
- [ ] Replace `RemoteTransport::read_output_line` with the chosen byte or
  framed-message read boundary.
- [ ] Update `SshRemoteTransport` to support the new transport boundary.
- [ ] Update `RemoteSession` to require `Hello` negotiation before frames.
- [ ] Update `tui-kit-cli` to use the new input writer and session events.
- [ ] Delete `TKF1` decoder and line-token input codec code instead of keeping
  fallback support.
- [ ] Replace current remote tests with typed framing/session tests listed
  above.
- [ ] Update remote module docs, lib docs, and public retention notes.
- [ ] Run `cargo fmt --all` in tui-kit.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [ ] Run `cargo test` in tui-kit.
- [ ] Run `cargo doc --no-deps` in tui-kit.
- [ ] Run
  `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  in tui-kit.
- [ ] Verify `git status --short -- visual-tests` in tui-kit still reports only
  the pre-existing untracked directory.
- [ ] Run c4tui fmt, clippy, and tests if dependency behavior or public names
  used by c4tui changed.
- [ ] Commit the completed Phase 22 work before starting Phase 23.

## Verification Commands

tui-kit:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui
git status --short -- visual-tests
```

c4tui, if dependency behavior changes:

```sh
cd /Users/coleshaffer/Projects/c4tui
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Commit Scope

One tui-kit commit should include:

- typed remote protocol model;
- length-prefixed framing;
- session negotiation changes;
- CLI protocol updates;
- remote tests and docs.

c4tui should have no source changes unless verification exposes a real local
dependency issue.
