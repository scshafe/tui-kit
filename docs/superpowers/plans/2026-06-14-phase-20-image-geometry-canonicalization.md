# Phase 20 - Image Geometry Canonicalization

**Status:** PLANNED - do not implement until Phase 19 is committed, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 19 c4tui terminal rendering and backend split.

## Goal

Make `tui_kit::layout::PlacementEngine` plus `ViewTransform` the canonical
model for c4tui pan, zoom, placement, and hit testing. c4tui should store
product state and explicit placement policy, then delegate geometry decisions
to one engine. `ImageViewportWidget` should remain the tui-kit raster/export
widget, but c4tui should not maintain a second persistent scale/offset model
and adapt it back into `ViewTransform`.

This phase must preserve current behavior. If exact canonicalization would
change visible placement, crop, pan, zoom, hit-testing, or status text, stop and
bring the difference back for operator approval before landing the change.

This phase must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase:

- `tui-kit/src/layout.rs` owns public geometry primitives,
  `PlacementPolicy`, `PlacementEngine`, `ViewTransform`, `Placement`, and
  `ImagePoint`.
- `ViewTransform` has convenience methods that construct the built-in
  `PlacementPolicy::crop_fit_centered()` engine internally.
- `tui-kit/src/widgets/image_viewport.rs` owns `ImageViewport`,
  `ImageViewportWidget`, `ImageViewportPlacement`, zoom/step state,
  resize policy, image export, and widget placement.
- `ImageViewport::placement` delegates pure placement geometry through a
  crate-private layout helper, but widget scale/offset remains a second
  geometry model.
- `c4tui/src/viewport_state.rs` stores both:
  - `HashMap<ViewId, ViewTransform>`;
  - `HashMap<ViewId, ImageViewportWidget>`.
- c4tui derives status placement by converting `ImageViewportPlacement` back
  into `tui_kit::layout::Placement`.
- c4tui hit testing calls `ImageViewportWidget::normalized_to_image`, while
  render/status placement calls widget placement.
- `Command::Zoom` carries a `ZoomAnchor`, but c4tui currently preserves
  center-anchored behavior by ignoring the anchor.

## Canonical Model

Use this ownership split:

- `tui_kit::layout` owns policy, transform, placement, panning, zooming, and
  canvas-to-image hit testing.
- `tui_kit::widgets::image_viewport` owns RGBA export, viewport image storage,
  widget resize policy, and terminal-widget materialization.
- `c4tui::ViewportState` owns per-view `ViewTransform` values and one explicit
  `PlacementEngine`.
- c4tui terminal rendering materializes an `ImageViewportWidget` only from the
  canonical transform/placement needed to export and place the current raster.

Planned c4tui state shape:

```rust
pub(crate) struct ViewportState {
    transforms: HashMap<ViewId, ViewTransform>,
    engine: PlacementEngine,
    policy: PlacementPolicy,
}
```

The final shape can omit `policy` if `PlacementEngine::policy()` provides every
needed read. Do not store persistent `ImageViewportWidget` values in c4tui
after this phase unless implementation proves that resize policy cannot be
represented by `ViewTransform` without a behavior change.

## Materializing Widgets

c4tui still needs `ImageViewportWidget` to export RGBA and call existing
tui-kit terminal image paths. Add the smallest final API needed to materialize
a widget from canonical geometry.

Preferred first attempt: implement a c4tui-private adapter using existing
public widget setters:

- build `ViewportImage` from the cached raster;
- create `ImageViewportWidget::from_image(image, canvas)`;
- set scale from `Placement::effective_scale`;
- compute scaled offset from `Placement::source`, `Placement::origin`, and
  `CanvasMetrics::cell_pixel`;
- set the offset;
- assert that `widget.placement()` is visually equivalent to the canonical
  placement in focused tests.

Only add a new tui-kit public constructor, such as
`ImageViewportWidget::from_placement`, if the private adapter would duplicate
toolkit-owned conversion rules or require access to private invariants. If a new
public constructor is added, it must be the final API, documented, covered by
tests, added to retention notes if needed, and verified by the public-surface
audit. Do not add a temporary compatibility helper.

## Pan, Zoom, and Hit Testing

Move c4tui interactions to `PlacementEngine`:

- `ViewportState::placement` should call `engine.place(image_size, canvas,
  transform)`.
- `ViewportState::zoom_view` should update the current transform through
  `engine.zoomed_at`.
- Preserve current center-anchored zoom behavior by passing the canvas center
  as the zoom anchor. Do not activate cursor-anchored zoom in this phase.
- `ViewportState::pan_view` should update the current transform through
  `engine.panned`.
- `ViewportState::element_at_canvas_point` and child-view hit testing should
  call `engine.canvas_to_image` using the same transform and placement model as
  rendering.
- `ViewportState::reset_viewport` should reset the transform to
  `ViewTransform::fit()`.
- `ViewportState::set_placement_policy` should rebuild the engine and clear
  derived transforms only where current behavior already clears them.

## Baseline Tests First

Before changing behavior, add tests that lock the current outcomes for:

- initial fit placement under the default c4tui policy;
- crop overflow zoom placement;
- overflow-cells placement;
- letterbox placement if current config exposes it;
- panning after zoom;
- reset after zoom/pan;
- hit testing with a zoomed source crop;
- current center-anchored zoom behavior, including the fact that
  `ZoomAnchor` is not used yet;
- widget materialization round trip from canonical placement to
  `ImageViewportWidget::placement`.

If any new canonical engine result differs from the baseline, classify it as
one of:

- an implementation bug to fix before landing;
- an existing test expectation that was too weak but behavior remains the same;
- a real behavior change that requires operator approval.

## ViewTransform Convenience Methods

Do not use `ViewTransform::place`, `ViewTransform::zoomed_at`,
`ViewTransform::panned`, or `ViewTransform::canvas_to_image` from c4tui after
this phase, because those methods bake in the default crop-fit-centered policy.
c4tui should always use its explicit configured `PlacementEngine`.

For tui-kit itself:

- keep the convenience methods only if they are treated as final default-policy
  helpers and retained by documentation/tests;
- otherwise remove them outright in this phase and update tests, docs, and
  retention notes;
- do not leave deprecated aliases or migration names.

Default plan: keep the methods for this phase unless the public-surface audit
shows they are stale and their removal is straightforward. The canonicalization
goal is to move c4tui off default-policy helpers, not to force unrelated public
API churn.

## Non-Goals

- No change to c4tui default placement policy.
- No cursor-anchored zoom behavior change.
- No terminal/backend restructuring; that belongs to Phase 19.
- No visual-test directory changes.
- No public compatibility shims.

## Implementation Tasks

- [ ] Confirm Phase 19 is implemented, verified, and committed.
- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 20 plan document if it is not already present in the
  implementation branch.
- [ ] Add baseline c4tui `ViewportState` tests for placement, pan, zoom, reset,
  hit testing, and center-anchored zoom behavior.
- [ ] Add tui-kit tests that prove canonical placement can materialize an
  `ImageViewportWidget` with visually equivalent placement.
- [ ] Refactor `c4tui::ViewportState` to store `ViewTransform` plus explicit
  `PlacementEngine` instead of persistent `ImageViewportWidget` values.
- [ ] Route c4tui placement, pan, zoom, reset, and hit testing through the
  engine.
- [ ] Add a private c4tui widget materialization helper, or a final tui-kit
  public constructor only if private materialization is not appropriate.
- [ ] Ensure c4tui root rendering still receives an `ImageViewportWidget` and
  PNG slice compatible with existing terminal rendering.
- [ ] Move c4tui off default-policy `ViewTransform` convenience methods.
- [ ] Decide, based on audit results, whether `ViewTransform` convenience
  methods remain documented final API or are removed outright.
- [ ] Update tui-kit retention docs if public layout/widget API changes.
- [ ] Run `cargo fmt --all` in tui-kit.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [ ] Run `cargo test` in tui-kit.
- [ ] Run `cargo doc --no-deps` in tui-kit.
- [ ] Run
  `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  in tui-kit.
- [ ] Verify `git status --short -- visual-tests` in tui-kit still reports only
  the pre-existing untracked directory.
- [ ] Run `cargo fmt --all` in c4tui.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [ ] Run `cargo test` in c4tui.
- [ ] Commit the completed Phase 20 work in each repo that changed before
  starting Phase 21.

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

tui-kit commit, if changed:

- canonical layout/widget materialization API or internal helper changes;
- layout/image viewport tests;
- public retention doc updates if the public surface changes.

c4tui commit:

- `ViewportState` canonical geometry refactor;
- c4tui placement, pan, zoom, reset, and hit-test tests;
- terminal integration updates required by the new viewport state shape.

