# Phase 16 - Terminal IO and Capability Probe Test Boundaries

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 15 image geometry boundary cleanup.

## Goal

Make terminal escape output and capability probing testable without a live
terminal. The production APIs should stay simple: apps still use
`Terminal`, `ImageSurfaceRegistry`, and `detect_capabilities`. The new seams
should be internal writer/reader boundaries that let tests assert exact bytes,
chunking, cleanup, and probe parsing deterministically.

This phase does not start Phase 17. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `tui-kit/src/image.rs` writes Kitty upload, place, delete, cursor movement,
  forget, shutdown, and flush directly through `io::stdout()`.
- `tui-kit/src/tty.rs` owns raw stdout helpers, but image lifecycle code still
  reaches global stdout itself.
- `c4tui/src/capabilities.rs` enters raw mode and reads/writes through
  process-global stdin/stdout directly.
- Capability parsing is tested, but query emission, probe cleanup, and
  read/write sequencing are not deterministic tests.

## Target Boundary

- Add a private tui-kit Kitty output trait with a production stdout
  implementation and test-only capture implementation.
- Keep `KittyImageRegistry` as the concrete public type and avoid public
  compatibility aliases or new app-facing constructors.
- Route all Kitty image bytes through the injected output, including
  `place_at`, replacement delete-before-place behavior, PNG chunking,
  `forget_all`, `shutdown`, and `flush`.
- Add a private c4tui capability probe IO trait that covers write, drain, read,
  and terminal restoration.
- Keep `detect_capabilities(timeout, force_probe)` as the public entrypoint.
- Add deterministic tests for capability query bytes, read completion, mouse
  probe cleanup, and restore-on-drop behavior.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Introduce a private tui-kit Kitty output trait and production stdout
  implementation in `src/image.rs`.
- [x] Route `KittyImageRegistry` upload/place/delete/forget/shutdown/flush
  through the injected output.
- [x] Add image tests that capture Kitty upload/place/delete/cursor/shutdown
  bytes, including multi-chunk upload behavior.
- [x] Introduce a private c4tui capability probe IO trait and production raw
  terminal implementation in `src/capabilities.rs`.
- [x] Route capability query writes, drains, reads, cleanup, and termios
  restoration through the injected probe IO.
- [x] Add capability tests for emitted probe bytes, scripted responses,
  cleanup, and restore-on-drop behavior.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Run `cargo doc --no-deps` in tui-kit because image/terminal internals
  touch public rustdoc-adjacent surfaces.
- [x] Run `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  in tui-kit to confirm no public-surface regression.
- [x] Verify `git status --short -- visual-tests` in tui-kit still reports
  only the pre-existing untracked directory.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Commit the completed Phase 16 work in each repo that changed.

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

tui-kit commit:

- this Phase 16 plan document;
- the private Kitty output boundary and deterministic image escape tests.

c4tui commit:

- the private capability probe IO boundary and deterministic probe tests.
