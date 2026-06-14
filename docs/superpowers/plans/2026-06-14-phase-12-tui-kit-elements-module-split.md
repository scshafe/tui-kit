# Phase 12 - tui-kit Elements Module Split

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 10 assessment and Phase 11 c4tui app orchestration extraction.

## Goal

Split `/Users/coleshaffer/Projects/tui-kit/src/elements/mod.rs` into focused
modules without changing behavior or preserving temporary compatibility names.
The public `tui_kit::elements::*` surface remains the final retained element
API, re-exported only from `elements::mod`.

This phase does not start Phase 13. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase, `src/elements/mod.rs` owns all of these responsibilities in
one file:

- core element traits and `ElementOutcome`;
- text rendering and padding/border configuration;
- `Panel`, `Stack`, and child container plumbing;
- `ScrollY`, `Focusable`, `KeyMapped`, `Padded`, and `Bordered`;
- `Window`, key-scope metadata, lifecycle hooks, cache stats, and grouped
  render-effect tracking;
- `ImageViewportElement`;
- `Modal`;
- `Overlay`;
- a large inline behavioral test suite.

`src/elements/effect.rs` already contains the data-only `RenderEffect` enum and
`EffectElement` trait with its focused unit test. Keep that module as-is unless
imports need to change.

## Target File Structure

Create:

- `/Users/coleshaffer/Projects/tui-kit/src/elements/core.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/decorators.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/containers.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/window.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/image.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/modal.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/overlay.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/tests.rs`

Modify:

- `/Users/coleshaffer/Projects/tui-kit/src/elements/mod.rs`
- `/Users/coleshaffer/Projects/tui-kit/src/elements/effect.rs` if needed for
  module-path cleanup only.
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-14-phase-12-tui-kit-elements-module-split.md`

Do not modify:

- `/Users/coleshaffer/Projects/tui-kit/visual-tests/`

## Public API Rules

- `elements::mod` is the public facade for retained final names.
- No deprecated re-exports, old module aliases, compatibility shims, or
  migration comments.
- New submodules stay private unless a public nested module is deliberately
  part of the final API. This phase does not add public nested modules.
- Retained public names:
  `EffectElement`, `Element`, `ElementExt`, `ElementOutcome`,
  `ContainerElement`, `RenderEffect`, `Text`, `TextOverflow`, `Padding`,
  `ElementBorder`, `Panel`, `Stack`, `StackConstraint`, `StackDirection`,
  `ScrollY`, `Focusable`, `KeyMapped`, `Padded`, `Bordered`, `Window`,
  `WindowChrome`, `WindowFocusScope`, `WindowLifecycleEvent`,
  `WindowRenderStats`, `WindowRepaintPolicy`, `KeyScope`, `KeyScopeResolver`,
  `KeyScopeRole`, `KeyResolution`, `ImageViewportElement`, `Modal`, and
  `Overlay`.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Move core traits, text rendering, `Padding`, and `ElementBorder` into
  `src/elements/core.rs`.
- [x] Move `ScrollY`, `Focusable`, `KeyMapped`, `Padded`, and `Bordered` into
  `src/elements/decorators.rs`.
- [x] Move `Panel`, `Stack`, shared child-element plumbing, and stack sizing
  into `src/elements/containers.rs`.
- [x] Move `Window`, key-scope types, window lifecycle types, and cache/effect
  placement tracking into `src/elements/window.rs`.
- [x] Move `ImageViewportElement` into `src/elements/image.rs`.
- [x] Move `Modal` into `src/elements/modal.rs`.
- [x] Move `Overlay` into `src/elements/overlay.rs`, reusing the shared child
  plumbing from `containers`.
- [x] Replace `src/elements/mod.rs` with module declarations, private imports
  only where needed, and final public re-exports.
- [x] Move the large inline element behavioral test suite into
  `src/elements/tests.rs`; keep `effect.rs` render-effect data-contract tests
  colocated with `RenderEffect`.
- [x] Add or preserve explicit tests for `ImageViewportElement` placement and
  grouped teardown behavior.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Run `cargo doc --no-deps` in tui-kit because this changes public module
  organization and rustdoc links.
- [x] Run `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`.
- [x] Verify `git status --short -- visual-tests` still reports only the
  pre-existing untracked directory.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Commit the completed Phase 12 work.

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

- this Phase 12 plan document;
- the `elements` module split;
- moved or adjusted tests needed to validate the split.

c4tui should have no source changes unless the public surface audit or build
exposes a real c4tui-facing break.
