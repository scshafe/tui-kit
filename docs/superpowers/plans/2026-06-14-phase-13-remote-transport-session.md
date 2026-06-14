# Phase 13 - Remote Transport and Session State Machine

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 12 tui-kit elements module split.

## Goal

Turn the Phase 9 `tui-kit-cli` remote launcher prototype into a testable remote
runtime boundary before adding any new remote features. The `TKF1` line protocol
stays unchanged. This phase adds transport and session ownership types around
the existing codec and moves direct SSH process construction out of the CLI.

This phase does not start Phase 14. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `src/remote.rs` defines protocol constants, `RemoteRunSpec`,
  `remote_ssh_args`, input event encode/decode helpers, `RemoteFrameDecoder`,
  `RemoteOutput`, and `decode_frame`.
- `src/bin/tui-kit-cli.rs` directly spawns `ssh`, takes child stdin/stdout,
  enters the local terminal before any remote frame is proven, starts an input
  writer thread, drives `RemoteFrameDecoder`, and waits for the child process.
- Existing `tests/remote.rs` cover argument parsing, input event round trips,
  full frame decoding, and streaming decoder passthrough behavior.

## Target Design

Add these public remote runtime types in `src/remote.rs`:

- `RemoteStderrPolicy` to make stderr handling explicit for process-backed
  transports.
- `RemoteExitStatus` as a renderer-neutral child-exit summary that tests can
  construct without spawning a process.
- `RemoteInputWriter` for typed local input forwarding.
- `RemoteLineInputWriter<W>` as the generic line writer used by SSH and tests.
- `RemoteTransport` for output-line reads, process-exit polling/waiting, and
  stderr policy reporting.
- `SshRemoteTransport` as the concrete process-backed transport.
- `RemoteSessionState`, `RemoteSessionEvent`, and `RemoteSession<T>` to own
  frame decoding and the startup/streaming/EOF/child-exit state transitions.

The CLI should:

- parse the same `tui-kit-cli run ssh://host -- app [args...]` shape;
- spawn `SshRemoteTransport` instead of `Command` directly;
- drive `RemoteSession` instead of calling `RemoteFrameDecoder` inline;
- enter the local terminal only after the first remote frame arrives;
- start the input writer only after the terminal has entered;
- stop and join the input writer helper when the remote session finishes.

## Non-Goals

- No new wire protocol version.
- No host profile/config format.
- No reconnection behavior.
- No persistent remote agent.
- No live SSH fixture in automated tests.
- No c4tui source changes unless verification exposes a real dependency break.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Add `RemoteStderrPolicy`, `RemoteExitStatus`, `RemoteInputWriter`, and
  `RemoteLineInputWriter<W>`.
- [x] Add `RemoteTransport` and `SshRemoteTransport::spawn`, reusing
  `remote_ssh_args` and preserving inherited stderr as the CLI default.
- [x] Add `RemoteSessionState`, `RemoteSessionEvent`, and `RemoteSession<T>`.
- [x] Move CLI process/session orchestration from direct `Command` usage to
  `SshRemoteTransport` plus `RemoteSession`.
- [x] Delay terminal entry until the first `RemoteSessionEvent::Frame`.
- [x] Stop and join the CLI input writer helper at the end of the remote run.
- [x] Add in-memory transport tests for passthrough-before-first-frame,
  first-frame transition, EOF while a frame is pending, malformed completed
  frames, non-zero child exit, and input forwarding.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Run `cargo doc --no-deps` in tui-kit because new public remote runtime
  types are added.
- [x] Run `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`.
- [x] Verify `git status --short -- visual-tests` still reports only the
  pre-existing untracked directory.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Commit the completed Phase 13 work.

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

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Commit Scope

One tui-kit commit should include:

- this Phase 13 plan document;
- remote transport/session runtime boundaries;
- CLI orchestration updates;
- focused remote session tests.

c4tui should have no source changes unless the local tui-kit dependency now
requires them.
