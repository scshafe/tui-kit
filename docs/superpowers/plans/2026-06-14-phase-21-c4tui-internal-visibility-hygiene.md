# Phase 21 - c4tui Internal Visibility Hygiene

**Status:** PLANNED - do not implement until Phase 20 is committed, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 20 image geometry canonicalization.

## Goal

Tighten c4tui internal visibility after the runtime, terminal, and geometry
boundaries settle. c4tui is a binary crate, so most top-level `pub` items are
not external API. Convert default internal surfaces to `pub(crate)` or private,
keep `pub` only where Rust requires it, and add an `unreachable_pub` check once
the codebase is clean enough for it to stay enabled.

This phase must preserve behavior and must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `c4tui/src/main.rs` declares all modules with private `mod` items.
- Many items inside those private modules are still `pub`, including app,
  backend, config, state, modal, render, workspace, terminal, statusbar,
  nav-picker, and id types.
- Some test-only re-exports already use `#[cfg(test)]`, such as workspace model
  details.
- `backend::fake` is `#[cfg(test)] pub mod fake`, but it is still more visible
  than the binary crate needs.
- There is no crate-level or CI-enforced `unreachable_pub` lint in c4tui.

## Target Boundary

- Use private visibility by default.
- Use `pub(crate)` for names shared across production modules.
- Use `pub(super)` for names shared only inside a module subtree, especially
  workspace and terminal submodules.
- Use `#[cfg(test)] pub(crate)` only for helpers that tests in sibling modules
  genuinely need.
- Keep child test modules where private access is the cleanest option.
- Avoid exposing a type solely to keep an old test path alive; move the test or
  helper instead.
- Add `#![warn(unreachable_pub)]` to the c4tui crate once all warnings are
  resolved.

No deprecated aliases, public compatibility re-exports, or migration modules are
allowed.

## Audit Strategy

Start with a non-mutating visibility audit:

```sh
cd /Users/coleshaffer/Projects/c4tui
RUSTFLAGS="-W unreachable_pub" cargo check --all-targets
```

Use the warning list to group changes by module. Do not apply a crate-level
lint until the module batches are clean.

Recommended batch order:

1. Leaf value modules: `ids`, `event`, `clipboard`, `logger`, `render`,
   `render_cache`, `render_pool`.
2. UI component modules: `nav_items`, `nav_picker`, `link_directory`,
   `log_view`, `modal`, `modal_factory`.
3. Runtime/app modules: `state`, `effect_runner` or `runtime`, `app`,
   `backend`.
4. Terminal modules from Phase 19.
5. Workspace subtree.
6. Config, CLI, capabilities, and main wiring.
7. Add the crate-level `#![warn(unreachable_pub)]` and run final verification.

## Module Notes

Likely changes, subject to the actual lint output:

- `App` can be `pub(crate)` because only `main.rs` and app tests construct it.
- `TerminalBackend` and the fake backend should be `pub(crate)` and
  `#[cfg(test)] pub(crate)` respectively.
- `AppState`, `RenderFrame`, `UpdateResult`, `Effect`, and `ModalSpec` can be
  `pub(crate)` unless Phase 18/19 tests can stay as child modules and use
  private access.
- Modal traits and type aliases should be `pub(crate)` or private. Type aliases
  such as `IntoNavTarget`, `HoveredViewFn`, `ThumbnailFn`, and `NavOnSelect`
  should become private if only `NavPickerModal` construction needs them.
- `NavPicker` internals such as `NavCellCanvas`, `ThumbnailId`, and
  `NavRenderArtifact` should be private or `pub(crate)` based on modal factory
  and terminal thumbnail usage.
- Workspace public facade re-exports in `workspace/mod.rs` should become
  `pub(crate) use`, with test-only model details behind
  `#[cfg(test)] pub(crate) use`.
- `statusbar` segment modules and builder types should be private unless tests
  or terminal/status modules require crate visibility.
- `image_id_for_view` should be `pub(crate)` or moved into the terminal image
  module from Phase 19 if that is its only production consumer.
- `Cli`, `Capabilities`, and config loader surfaces should be `pub(crate)`.

## Tests

Prefer moving tests toward the owner of the private behavior instead of making
implementation details crate-visible for convenience.

Allowed test visibility patterns:

- child `mod tests` for private helper coverage;
- `#[cfg(test)] mod tests;` sibling test files for large module tests;
- `#[cfg(test)] pub(crate) mod test_support` only for genuinely reused fixture
  builders.

Do not promote c4tui test helpers to tui-kit in this phase.

## Non-Goals

- No behavior changes.
- No terminal/runtime/geometry refactors.
- No test fixture architecture cleanup beyond visibility-driven movement; that
  is Phase 23.
- No tui-kit public API changes.

## Implementation Tasks

- [ ] Confirm Phase 20 is implemented, verified, and committed.
- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 21 plan document if it is not already present in the
  implementation branch.
- [ ] Run `RUSTFLAGS="-W unreachable_pub" cargo check --all-targets` in c4tui
  and capture the warning groups locally.
- [ ] Convert leaf value modules from `pub` to private or `pub(crate)` as
  appropriate.
- [ ] Convert UI component and modal modules from `pub` to private or
  `pub(crate)`.
- [ ] Convert app/runtime/backend surfaces from `pub` to private or
  `pub(crate)`.
- [ ] Convert terminal module surfaces from `pub` to private, `pub(super)`, or
  `pub(crate)` according to the Phase 19 file split.
- [ ] Convert workspace subtree exports to `pub(crate)` or `pub(super)`.
- [ ] Convert config, CLI, capability, logger, and main wiring surfaces.
- [ ] Move tests or add `#[cfg(test)] pub(crate)` exports only where private
  child tests are not practical.
- [ ] Add `#![warn(unreachable_pub)]` to c4tui once the audit is clean.
- [ ] Run `cargo fmt --all` in c4tui.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [ ] Run `cargo test` in c4tui.
- [ ] Run tui-kit verification only if dependency behavior changes
  unexpectedly.
- [ ] Commit the completed Phase 21 work before starting Phase 22.

## Verification Commands

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
RUSTFLAGS="-W unreachable_pub" cargo check --all-targets
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

tui-kit, only if touched:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short -- visual-tests
```

## Commit Scope

One c4tui commit should include:

- visibility narrowing;
- any test relocations required by privacy;
- crate-level `unreachable_pub` warning enablement once clean.

tui-kit should have no source changes in this phase.

