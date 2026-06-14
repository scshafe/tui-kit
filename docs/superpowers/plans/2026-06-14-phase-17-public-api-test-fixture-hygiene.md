# Phase 17 - Public API and Test Fixture Hygiene

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 16 terminal IO and capability probe test boundaries.

## Goal

Finish the Phase 10 hygiene follow-on by removing stale compiler allowances,
keeping only intentional public names, and moving oversized test fixtures away
from production orchestration files where that improves maintainability without
changing behavior.

This phase is intentionally conservative: it does not start new architecture
work beyond hygiene and fixture placement, and it must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `tui-kit` has no remaining `dead_code`, `unused_imports`, or
  `too_many_arguments` allowances in the active source tree.
- `c4tui` still has broad module-level `dead_code` allowances in logging,
  clipboard, navigation, status bar, keymap, and nav-item modules.
- `c4tui` still has narrow local allowances around app and terminal builder
  helpers plus an unused public re-export of tui-kit's input event type.
- `c4tui/src/app.rs` contains a large inline test module after the production
  app orchestration code, making the file harder to review after the Phase 11
  orchestration split.

## Target Boundary

- Remove broad allowance markers instead of using them as retention notes.
- Delete unused local API surfaces where the codebase no longer has a real
  caller.
- Keep retained public names only where they are part of the intended final
  local API, not as compatibility aliases or migration shims.
- Move the large app test module to `c4tui/src/app/tests.rs` so production
  orchestration and test fixtures can evolve independently.
- Keep reusable test helpers local until there is clear cross-module reuse.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Move the large `c4tui/src/app.rs` inline test module to
  `c4tui/src/app/tests.rs` and leave only `#[cfg(test)] mod tests;` in the
  production file.
- [x] Remove stale `dead_code` allowances from c4tui modules in small batches.
- [x] Remove stale local dead-code allowances from c4tui app, terminal, and main
  helper surfaces.
- [x] Remove the unused `TuiKitInputEvent` public re-export from c4tui.
- [x] Let `clippy -D warnings` identify any genuinely unused items exposed by
  the removed allowances; delete unused surfaces or narrow them behind tests
  only when the tests are the real owner.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Run `cargo doc --no-deps` in tui-kit because this phase touches public API
  organization and audit documentation.
- [x] Run
  `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  in tui-kit.
- [x] Verify `git status --short -- visual-tests` in tui-kit still reports only
  the pre-existing untracked directory.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Commit the completed Phase 17 work in each repo that changed.

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

- this Phase 17 plan document.

c4tui commit:

- app test-module relocation;
- removal of stale allowance markers;
- deletion or narrowing of any newly exposed unused local surfaces;
- removal of the unused input-event re-export.
