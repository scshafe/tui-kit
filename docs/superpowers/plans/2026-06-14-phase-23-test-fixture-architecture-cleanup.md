# Phase 23 - Test Fixture Architecture Cleanup

**Status:** PLANNED - do not implement until Phase 22 is committed, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 22 remote protocol typed framing.

## Goal

Clean up test fixture architecture after the main runtime, terminal, geometry,
visibility, and remote protocol boundaries have settled. Keep test helpers
local by default. Promote helpers only when reuse is real and the helper is
owned by the right crate boundary.

This phase must preserve behavior and must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `c4tui/src/app/tests.rs` contains multiple app builders that create temporary
  SVG files, catalogs, render caches, viewport state, log buffers, and
  clipboards.
- `c4tui/src/effect_runner.rs` or the Phase 18 runtime tests contain a local
  harness with similar catalog/cache/viewport/log setup.
- `c4tui/src/state.rs` and `c4tui/src/viewport_state.rs` tests each define
  canvas helpers, raster budgets, SVG fixtures, catalog builders, and rendered
  view fixtures.
- `c4tui/src/workspace/tests.rs` repeats temporary workspace JSON/SVG setup.
- `c4tui/src/link_directory.rs`, `modal_factory.rs`, `view_catalog.rs`,
  `terminal.rs`, and `statusbar.rs` all carry local model/view builders.
- `tui-kit/src/testkit.rs` already exposes public helpers for toolkit-level
  widget rendering, event scripts, image lifecycle assertions, renderer backend
  assertions, and deterministic scheduler work.
- `docs/testing-patterns.md` says c4tui app lifecycle assertions belong in
  c4tui, while toolkit trait/buffer/effect/scheduler assertions belong in
  tui-kit.

## Target Boundary

Add private c4tui test support for repeated app/domain fixtures, while keeping
one-off test setup next to the test that owns it.

Planned c4tui structure:

- `src/test_support/mod.rs`
- `src/test_support/app.rs`
- `src/test_support/catalog.rs`
- `src/test_support/render.rs`
- `src/test_support/workspace.rs`
- `src/test_support/events.rs`

Declare from `main.rs`:

```rust
#[cfg(test)]
mod test_support;
```

All c4tui test support must be `pub(crate)` or narrower and compiled only for
tests. Do not expose it as a library API.

Promote to `tui_kit::testkit` only when the helper:

- works entirely through tui-kit public traits or types;
- is not c4tui-specific;
- has at least two in-tree users or one strong cross-crate use;
- can be documented as stable test infrastructure;
- is verified by cargo doc and the public-surface audit.

## c4tui Fixture Candidates

Consolidate repeated c4tui setup into private builders:

- `TestSvg`: writes small SVG files to a temp directory with optional element
  rectangles.
- `TestViewBuilder`: creates `ViewInfo` with key, name, kind, SVG path,
  element ids, child-view links, primary view, and key view fields.
- `TestModelBuilder`: creates `WorkspaceModel`, `ElementMetadata`,
  `RelationshipMetadata`, and relationship indexes.
- `TestCatalogBuilder`: creates `ViewCatalog` with optional model and exported
  workspace.
- `TestRenderParts`: creates `(ViewCatalog, RenderCache, ViewportState)` using
  a deterministic `RasterBudget`.
- `AppHarness`: owns temp directory, app, fake terminal, and event sink for app
  lifecycle tests.
- `RuntimeHarness`: owns the runtime executor fields after Phase 18.
- `WorkspaceJsonFixture`: writes workspace JSON plus matching SVG files.
- `run_keys` and event-script helpers for c4tui app tests. Prefer using
  `tui_kit::testkit::EventScript` if it fits without c4tui-specific wrapping.

Do not consolidate tiny one-test helpers if the abstraction would obscure the
test.

## tui-kit Testkit Audit

Audit `tui-kit/src/testkit.rs` after the c4tui cleanup:

- keep helpers that are used by tui-kit tests, c4tui tests, or documented
  retention notes;
- prune stale helpers if they have no real consumer and are not retained by a
  documented final API reason;
- add retention notes for any newly promoted helper;
- avoid adding c4tui domain builders, Structurizr fixtures, or app lifecycle
  harnesses to tui-kit.

Potential library-generic promotions, only if implementation proves reuse:

- a small SVG/RGBA image fixture helper for image viewport tests;
- a generic temp text-file helper if both repos independently need it;
- additional `RenderEffect` assertion helpers if multiple tui-kit modules use
  the same matching logic.

Default plan: do not promote anything to tui-kit unless the c4tui cleanup makes
the cross-crate value obvious.

## Test Movement

Move tests only when it improves ownership:

- Large c4tui module tests can remain as `#[cfg(test)] mod tests;` sibling
  files if they need private module access.
- Shared fixture code should move to `test_support`; assertions should stay in
  the behavior-owning test module.
- Do not make production types more visible solely to support moved tests.
- Do not convert unit tests to integration tests if that would force public
  visibility.

## Non-Goals

- No behavior changes.
- No runtime, terminal, geometry, visibility, or remote protocol refactors.
- No c4tui library crate conversion.
- No visual-test changes.
- No public testkit expansion without documented reuse and public-surface
  verification.

## Implementation Tasks

- [ ] Confirm Phase 22 is implemented, verified, and committed.
- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 23 plan document if it is not already present in the
  implementation branch.
- [ ] Inventory repeated c4tui fixtures across app, runtime/effect, state,
  viewport, workspace, modal, link-directory, view-catalog, terminal, and
  statusbar tests.
- [ ] Add `#[cfg(test)] mod test_support;` to c4tui and create the planned
  private test support modules.
- [ ] Move repeated SVG/temp-dir helpers into `test_support`.
- [ ] Move repeated `ViewInfo`, `WorkspaceModel`, relationship, and catalog
  builders into `test_support`.
- [ ] Move repeated app/runtime harness setup into `test_support` only where at
  least two test modules use it.
- [ ] Update c4tui tests to use the private fixtures while keeping assertions
  local and readable.
- [ ] Remove fixture helpers that became unused after consolidation.
- [ ] Audit tui-kit `testkit` usage and retention notes.
- [ ] Promote only genuinely toolkit-generic helpers to `tui_kit::testkit`, if
  any; otherwise leave tui-kit source unchanged.
- [ ] Update `docs/testing-patterns.md` if fixture ownership rules or examples
  change.
- [ ] Run `cargo fmt --all` in c4tui.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [ ] Run `cargo test` in c4tui.
- [ ] Run `cargo fmt --all` in tui-kit.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [ ] Run `cargo test` in tui-kit.
- [ ] If tui-kit `testkit` public surface changes, run `cargo doc --no-deps`
  and
  `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`.
- [ ] Verify `git status --short -- visual-tests` in tui-kit still reports only
  the pre-existing untracked directory.
- [ ] Commit the completed Phase 23 work in each repo that changed.

## Verification Commands

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

tui-kit:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short -- visual-tests
```

If `tui_kit::testkit` public API changes:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo doc --no-deps
scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui
```

## Commit Scope

c4tui commit:

- private `test_support` modules;
- fixture consolidation;
- test updates only.

tui-kit commit, only if needed:

- library-generic testkit helper changes;
- testing-pattern documentation updates;
- public retention updates if public testkit surface changes.

