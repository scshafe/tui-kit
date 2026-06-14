# Phase 6 - Split c4tui ViewStore

## Goal

Replace the monolithic `c4tui::view::ViewStore` with explicit owners for the three responsibilities it currently mixes:

- `ViewCatalog`: exported view metadata, workspace model data, relationship navigation metadata, and the exported workspace lifetime guard.
- `RenderCache`: rendered SVG rasters, raster budget, render job discovery, and scheduler merge points.
- `ViewportState`: image viewport widgets, transforms, placement policy, hit testing, zoom, pan, and viewport reset behavior.

`ViewStore` should not remain as a type alias or compatibility wrapper. Call sites should receive the narrowest state they need.

## Implementation Plan

1. Add `view_catalog`, `render_cache`, and `viewport_state` modules in `c4tui`.
2. Move catalog-only data and relationship traversal from `ViewStore` into `ViewCatalog`.
3. Move rendered raster ownership and render-job bookkeeping into `RenderCache`.
4. Move viewport widgets, transforms, placement-policy handling, and canvas hit-testing into `ViewportState`.
5. Update `App`, `AppState`, `TerminalBackend`, `TerminalSession`, `RenderScheduler`, and `LinkDirectory` to use explicit catalog/cache/viewport references.
6. Remove `ViewStore` entirely and keep helper APIs in the owning modules.
7. Move existing `ViewStore` tests to the new module boundaries, preserving behavior coverage for catalog validation, relationship candidates, child-view hit testing, zoom, and panning.
8. Update docs and comments that still mention `ViewStore`.

## Verification

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `tui-kit` workspace verification remains unchanged; run its test/clippy/format loop after committing the c4tui refactor.
