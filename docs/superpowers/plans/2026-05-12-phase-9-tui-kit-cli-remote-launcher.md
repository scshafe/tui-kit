# Phase 9 - tui-kit-cli Remote Launcher Prototype

**Status:** ACTIVE - 2026-06-13
**Repo for code:** `/Users/coleshaffer/Projects/tui-kit`
**Cross-repo verification:** `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` Phase 9.
**Predecessor:** Phase 8 committed as `96e5e41`.

## Goal

Add a small `tui-kit-cli` prototype that launches a compatible remote app over
`ssh -tt`, forwards typed local input events, and renders line-delimited remote
buffer/effect frames through the Phase 8 renderer backend.

This phase intentionally does not add host profiles, a daemon, arbitrary local
command execution, screen capture, or a stable wire protocol.

## Protocol Shape

- Launcher command:
  `tui-kit-cli run ssh://host -- app [args...]`
- SSH command shape:
  `ssh -tt host env TUI_KIT_REMOTE_RENDER=1 TUI_KIT_REMOTE_PROTOCOL=tui-kit-remote-v1 app [args...]`
- Input events sent to remote stdin as one typed line per event:
  - `key char <u32>`
  - `key up|down|left|right|enter|tab|back|esc|ctrl-c|unknown`
  - `mouse click|drag|wheel-up|wheel-down <x> <y>`
  - `mouse release`
  - `resize <cols> <rows>`
- Remote frames read from stdout:
  - `TKF1 <cols> <rows>`
  - `TEXT <row> <base64-utf8>`
  - `EFFECT ...`
  - `END`

Plain `ssh -t host app` remains the fallback for apps that choose ordinary
terminal mode instead of this prototype protocol.

## File Structure

- `src/remote.rs` - protocol constants, CLI parsing helpers, input codec,
  frame decoder, and SSH argument builder.
- `src/bin/tui-kit-cli.rs` - launcher binary.
- `src/lib.rs` - expose `remote`.
- `tests/remote.rs` - protocol and parser tests.
- `README.md`, `specification.md`, `architecture.md` - document the prototype
  and its limits.

## Tasks

- [x] **Task 1 - Remote protocol helpers**
  - Add `src/remote.rs`.
  - Define protocol/env constants.
  - Define `RemoteRunSpec` and parse `run ssh://host -- app` arguments.
  - Build explicit `ssh -tt host env ...` args.

- [x] **Task 2 - Typed input codec**
  - Encode `InputEvent::{Key, Mouse, Resize}` as line-delimited protocol
    messages.
  - Add a symmetric decoder for tests and future compatible remote apps.

- [x] **Task 3 - Remote frame decoder**
  - Decode `TKF1` frame lines into `renderer::RenderFrame`.
  - Support text rows and the current `RenderEffect` variants.
  - Add a streaming decoder that distinguishes passthrough lines from complete
    frames.

- [x] **Task 4 - CLI binary**
  - Add `src/bin/tui-kit-cli.rs`.
  - Spawn `ssh` with piped stdin/stdout and inherited stderr.
  - Enter local `Terminal`, spawn the input thread, forward typed input to
    remote stdin, decode remote frames, and render them through
    `RendererBackend`.
  - Surface malformed protocol or failed child exit status as non-zero errors.

- [x] **Task 5 - Tests**
  - Add `tests/remote.rs` covering argument parsing, ssh args, input
    encode/decode, frame decode, and streaming decoder behavior.
  - No live SSH test is required in CI.

- [x] **Task 6 - Documentation**
  - Update README/spec/architecture with the prototype command, env handshake,
    protocol limits, and plain-SSH fallback note.

- [x] **Task 7 - Verification**
  - In `tui-kit`:
    - `cargo fmt --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
    - `cargo doc --no-deps`
    - `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  - In `c4tui`:
    - `cargo fmt --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
