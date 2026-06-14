# Phase 15 - Image Geometry Boundary Cleanup

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 14 c4tui workspace module split.

## Goal

Sharpen the ownership boundary between tui-kit's pure image layout math and the
stateful image viewport widget. `src/layout.rs` should own deterministic
placement geometry, validation, and golden placement cases. `src/widgets/
image_viewport.rs` should keep viewport state transitions, resize policy, and
raster export.

This phase does not start Phase 16. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `src/layout.rs` owns the public `PlacementEngine`, `PlacementPolicy`,
  `ViewTransform`, `TailViewport`, and pixel/cell geometry primitives.
- `src/layout.rs` has a module-level `#![allow(dead_code)]`.
- `src/layout.rs` uses a narrow `clippy::too_many_arguments` allowance around
  clipped-side calculation.
- `src/widgets/image_viewport.rs` owns interactive viewport state, widget resize
  policy, raster export, and terminal placement calculation.
- `src/widgets/image_viewport.rs` has pure placement helpers for crop
  calculation, source rectangle conversion, cell rounding, and destination
  origin calculation.
- `src/widgets/image_viewport.rs` uses a narrow `clippy::too_many_arguments`
  allowance around one-pixel RGBA copy.

## Target Boundary

- Keep all current public image viewport names and behavior.
- Move pure viewport placement geometry from `image_viewport` into `layout`
  behind a crate-private contract, avoiding new public compatibility surface.
- Keep state transitions, scaling validation, resize policy, image export, and
  `ImageViewportPlacement` conversion in `image_viewport`.
- Replace broad or argument-count allowances with small private structs or
  retained code paths that clippy can validate directly.
- Add golden placement cases in `layout` and keep widget-facing placement tests
  in `image_viewport` so both sides validate the same crop/cell behavior.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Add a crate-private layout placement result for scaled image viewport
  geometry: source crop, destination origin, destination cells, visible pixels,
  and theoretical pixels.
- [x] Move viewport crop math, source rectangle conversion, destination cell
  rounding, and empty-placement handling into `src/layout.rs`.
- [x] Update `ImageViewport::placement` to compute scale/theoretical size from
  state, delegate pure placement geometry to `layout`, and wrap the result in
  `ImageViewportPlacement`.
- [x] Keep `ImageViewportWidget`, resize policy, zoom/pan state transitions,
  and raster export behavior in `image_viewport`.
- [x] Add layout golden tests for visible crop, zoomed crop, centered smaller
  image, and off-canvas empty placement cases.
- [x] Preserve image viewport placement tests as state/wrapper tests against
  the same golden outcomes.
- [x] Remove the module-level `#![allow(dead_code)]` from `src/layout.rs`, or
  replace it only with targeted retention documentation if verification proves
  a retained private helper requires it.
- [x] Replace `clippy::too_many_arguments` allowances in `layout` and
  `image_viewport` with private request/context structs.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Run `cargo doc --no-deps` in tui-kit because this touches public API
  organization and rustdoc-adjacent ownership.
- [x] Run `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  in tui-kit to confirm no public-surface regression.
- [x] Verify `git status --short -- visual-tests` in tui-kit still reports
  only the pre-existing untracked directory.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Commit the completed Phase 15 work.

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

- this Phase 15 plan document;
- the layout/image viewport boundary cleanup;
- focused placement geometry tests and allowance removal.

c4tui should have no source changes unless verification exposes a real
c4tui-facing behavior or dependency issue.
