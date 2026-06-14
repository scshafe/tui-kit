# Phase 19 - c4tui Terminal Rendering and Backend Split

**Status:** PLANNED - do not implement until Phase 18 is committed, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 18 c4tui runtime transaction boundary.

## Goal

Split c4tui terminal rendering into focused internal units and narrow the
testing backend around high-level render transactions. `TerminalSession` should
remain the production terminal owner, but root canvas rendering, modal
rendering, status/footer rendering, image placement painting, and dialog/help
presentation should no longer live in one broad implementation file.

This phase must preserve behavior. It must not start Phase 20 geometry work and
must not touch `/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `c4tui/src/backend.rs` defines one broad `TerminalBackend` trait with canvas
  metrics, root render, root image teardown, modal render, modal close, image
  cache clear, message dialog, error dialog, and help dialog methods.
- `backend::fake::FakeTerminalBackend` records low-level calls such as
  `Render`, `RenderModal`, `CloseModal`, `ShowHelp`, and `ClearImageCache`.
- `c4tui/src/terminal.rs` defines `TerminalSession` and owns:
  - terminal entry;
  - root layout;
  - status/footer rendering;
  - image viewport placement;
  - modal pre-render placement cleanup;
  - modal buffer rendering;
  - modal thumbnail painting and stale-thumbnail cleanup;
  - dialogs and help text;
  - image cache clearing;
  - log view rendering helpers.
- `TerminalSession::render_modal_inner` still enumerates all catalog views to
  clear inactive thumbnail placement ids after a modal render.

## Target File Structure

Restructure c4tui terminal code into a module directory:

- `src/terminal/mod.rs`
- `src/terminal/session.rs`
- `src/terminal/root.rs`
- `src/terminal/modal.rs`
- `src/terminal/status.rs`
- `src/terminal/images.rs`
- `src/terminal/dialogs.rs`
- `src/terminal/log_view.rs`

The exact split can be adjusted while implementing, but the ownership should
remain:

- `session.rs`: production `TerminalSession` owner and public entrypoint for
  the rest of c4tui.
- `root.rs`: root canvas draw, root area layout, current view image placement,
  and status/footer composition.
- `status.rs`: conversion from `RenderFrame`, catalog, cache, viewport, and
  workspace path into status/footer strings.
- `modal.rs`: modal buffer draw plus typed modal image side effects.
- `images.rs`: mapping from `ViewId` to tui-kit image ids and placement ids,
  thumbnail painting, stale thumbnail cleanup, root viewport teardown, cache
  clear.
- `dialogs.rs`: message, error, and help presentation.
- `log_view.rs`: current `render_log_view` helper and its tests.

Do not move logic into tui-kit in this phase unless the implementation proves
the code is toolkit-generic. The default assumption is that this is c4tui
application rendering.

## Backend Boundary

After Phase 18, runtime transactions should be the main testable behavior. Use
that to narrow `TerminalBackend` rather than preserving the current broad
method list.

Preferred design:

```rust
pub(crate) trait TerminalBackend {
    fn canvas_metrics(&self) -> CanvasMetrics;
    fn execute(&mut self, command: TerminalCommand<'_>) -> Result<()>;
}

pub(crate) enum TerminalCommand<'a> {
    RenderRoot(RootRenderRequest<'a>),
    RenderModal(ModalRenderRequest<'a>),
    CloseModal(ModalCloseRequest<'a>),
    TeardownRootViewport { view_id: ViewId },
    ClearImageCache,
    ShowMessage { title: &'a str, message: &'a str },
    ShowError { title: &'a str, message: &'a str },
    ShowHelp { keys: &'a KeyBindings },
}
```

If lifetimes make this shape noisy, split into small traits instead:

- `CanvasMetricsProvider`;
- `RootRenderer`;
- `ModalRenderer`;
- `ImageCacheController`;
- `DialogPresenter`.

The final boundary should still let `FakeTerminalBackend` record high-level
commands without inheriting root layout, modal thumbnail, and dialog internals.
Do not keep deprecated aliases for old trait methods.

## Modal Rendering

Convert modal image side effects into a typed render result before the terminal
layer paints images.

Planned internal types:

```rust
pub(crate) struct ModalRenderPlan {
    pub pre_clear_placements: Vec<u32>,
    pub clear_all_placements: bool,
    pub thumbnails: Vec<ThumbnailCellArea>,
}

pub(crate) struct ThumbnailPaintPlan {
    pub active: Vec<ThumbnailCellArea>,
    pub stale: Vec<(u32, u32)>,
}
```

The modal still owns its current hooks:

- `pre_render_placements_to_clear`;
- `clear_all_placements_pre_render`;
- `post_render_thumbnails`.

But `TerminalSession` should not inline every step. `modal.rs` should collect
the modal plan, and `images.rs` should paint active thumbnails and clear stale
thumbnail placements. Existing behavior must remain:

- log view clears the main placement and tolerates `delete_all_placements`
  failure;
- picker thumbnails repaint after modal buffer draw;
- stale picker placements are torn down so removed thumbnails do not remain;
- `close_modal` tears down all per-view picker placement ids.

## Root Rendering

Move `TerminalSession::render_view` into a root renderer that:

- computes the current canvas metrics once;
- obtains the current c4tui placement through `ViewportState`;
- builds status/footer text through a status helper;
- draws status row, clear canvas, and footer row;
- renders the image viewport into the canvas area;
- flushes image operations.

No Phase 20 canonical geometry changes are allowed here. Root rendering should
keep using the current `ViewportState` and `ImageViewportWidget` behavior.

## Tests

Preserve the existing app tests that assert `FakeTerminalCall` order. After the
backend split, update those assertions to the new high-level command records
only where the backend boundary changed.

Add or keep focused tests for:

- root render command records current view id;
- modal render command records modal render without requiring real terminal
  image operations;
- close modal command records thumbnail cleanup intent;
- help/error/message commands remain distinct;
- modal stale-thumbnail cleanup computes inactive picker placements from the
  catalog;
- log modal still requests all-placement cleanup and main-placement cleanup;
- `pinned_connection_counts` remains unchanged after moving status/root code.

## Non-Goals

- No runtime transaction changes except adapting to the new backend interface.
- No c4tui geometry behavior changes.
- No tui-kit terminal API changes unless a bug is found.
- No public c4tui API commitment; c4tui is still a binary crate and internal
  names should be `pub(crate)` or private where possible.

## Implementation Tasks

- [ ] Confirm Phase 18 is implemented, verified, and committed.
- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 19 plan document if it is not already present in the
  implementation branch.
- [ ] Convert `src/terminal.rs` into a module directory with the target files
  above.
- [ ] Move root render logic out of `TerminalSession` into `terminal/root.rs`.
- [ ] Move status/footer context assembly into `terminal/status.rs`, keeping
  `statusbar.rs` as the reusable status bar component module.
- [ ] Move image id, root placement, thumbnail painting, stale thumbnail
  cleanup, modal close cleanup, and image cache clearing into
  `terminal/images.rs`.
- [ ] Move message/error/help dialog presentation into `terminal/dialogs.rs`.
- [ ] Move log-view buffer rendering into `terminal/log_view.rs`.
- [ ] Replace the broad `TerminalBackend` method list with a narrower command
  or trait-composition boundary.
- [ ] Update `TerminalSession` to implement the new backend boundary by
  delegating to focused renderers.
- [ ] Update `FakeTerminalBackend` to record high-level terminal commands and
  retain the existing assertion affordances needed by app/runtime tests.
- [ ] Add modal-render-plan and thumbnail-cleanup tests.
- [ ] Run `cargo fmt --all` in c4tui.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [ ] Run `cargo test` in c4tui.
- [ ] Run tui-kit verification only if implementation unexpectedly touches
  tui-kit or dependency behavior.
- [ ] Commit the completed Phase 19 work before starting Phase 20.

## Verification Commands

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
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

- terminal module split;
- narrowed backend boundary;
- fake backend updates;
- focused terminal-rendering tests.

tui-kit should have no source changes in this phase unless verification exposes
a real toolkit boundary issue.

