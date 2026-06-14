# Phase 8 - RenderEffect and Renderer Backend Model

**Status:** ACTIVE - 2026-06-13
**Repo for code:** `/Users/coleshaffer/Projects/tui-kit`
**Cross-repo verification:** `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` Phase 8.
**Predecessor:** Phase 7 committed as `740c5a3` in tui-kit and `f6152ec` in c4tui.

## Goal

Make tui-kit's render/effect boundary explicit without adding SSH, subprocess,
or wire-protocol behavior. The end state is a documented renderer backend trait
that consumes one buffer plus its render effects, with a local terminal adapter
and an in-process mock backend proving the contract.

## File Structure

- `src/renderer.rs` - new renderer contract:
  - `RenderFrame`
  - `RendererBackend`
  - `ImageSurfaceRenderer<S>`
- `src/terminal.rs` - implement `RendererBackend` for the existing local
  `Terminal` wrapper and add `Terminal::render_frame`.
- `src/testkit.rs` - add `MockRendererBackend`.
- `tests/renderer.rs` - integration tests for public renderer APIs and the
  mock/surface adapters.
- `src/lib.rs`, `README.md`, `specification.md`, `architecture.md`,
  `docs/testing-patterns.md` - docs and module map updates.

## Tasks

- [x] **Task 1 - Add renderer frame and backend trait**
  - Create `src/renderer.rs`.
  - Define `RenderFrame { buffer: Buffer, effects: Vec<RenderEffect> }`.
  - Add accessors: `new`, `empty`, `area`, `buffer`, `effects`, `into_parts`.
  - Define `RendererBackend` with `fn render_frame(&mut self, frame:
    &RenderFrame) -> anyhow::Result<()>`.

- [x] **Task 2 - Add image-surface renderer adapter**
  - Add `ImageSurfaceRenderer<S: ImageSurface>` in `src/renderer.rs`.
  - Store the last rendered buffer so tests can assert the backend consumed
    both halves of the frame.
  - Apply every `RenderEffect` to the wrapped `ImageSurface`.
  - Expose `surface`, `surface_mut`, `last_buffer`, and `into_surface`.

- [x] **Task 3 - Wire local terminal rendering**
  - Add `Terminal::render_frame(&mut self, frame: &RenderFrame)`.
  - Merge the frame buffer into ratatui's frame buffer.
  - Apply effects through `RenderEffect::apply_to_registry` so image origins
    are preserved.
  - Flush the image registry.
  - Implement `RendererBackend for Terminal`.

- [x] **Task 4 - Add in-process mock renderer**
  - Add `MockRendererBackend` to `testkit`.
  - Record cloned `RenderFrame` values and flattened `RenderEffect` values.
  - Implement `RendererBackend` for it.

- [x] **Task 5 - Public API tests**
  - Add `tests/renderer.rs`.
  - Test `MockRendererBackend` records buffers and effects.
  - Test `ImageSurfaceRenderer<MockImageSurface>` applies upload/place/delete
    effects.
  - Test `RenderFrame::into_parts` preserves buffer/effect data.

- [x] **Task 6 - Documentation**
  - Update README/spec/architecture module maps and rendering sections.
  - Update `docs/testing-patterns.md` to mention mock renderer tests.
  - Keep remote rendering described as future work; this phase ships only the
    in-process contract and local adapter.

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
