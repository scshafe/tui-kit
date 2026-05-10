# Codex Session Summary: 019e1141-720c-7ec3-8edc-484a4f8efdd3

Session file:
`~/.codex/sessions/2026/05/10/rollout-2026-05-10T02-39-22-019e1141-720c-7ec3-8edc-484a4f8efdd3.jsonl`

Approximate active window:
`2026-05-10 02:40 PDT` through `2026-05-10 07:17 PDT`.

Initial cwd:
`/Users/coleshaffer/Projects`

## High-Level Arc

This session continued the `tui-kit` image viewport/ImageBox work after prior
agents had failed to fix the zoom/crop behavior. The core conceptual target was:

1. Source image has real pixel dimensions.
2. A theoretical image is derived by applying one uniform zoom/scale.
3. A widget box acts as a crop aperture over that theoretical image.
4. Terminal cell/pixel details must not feed back into theoretical image math.

The session first avoided code edits and aligned on the model. It then built a
visual fixture and pixel-comparison test scaffolding, implemented the new
`ImageViewport` interface, updated `c4tui` to consume it, added build-time
stamps, added a reinstall script, diagnosed the remaining Kitty placement bug,
and finally moved the remaining low-level Kitty placement boundary into
`tui-kit`.

## Major Work Performed

### 1. Concept Alignment Before Edits

The user explicitly required no concept-oriented edits before discussion.
The agent paused, inspected the existing `ImageBox` implementation/tests, and
reported that focused `widgets::image_box` tests already passed.

The key invariant identified was:

- `theoretical = source_size * zoom`
- `offset` lives in theoretical/scaled-image pixel space
- `visible = intersection(theoretical image, widget aperture)`
- `source_crop = visible_theoretical_rect / zoom`
- terminal cell rounding is a final rendering concern only

### 2. Visual Fixture and Pixel Test Scaffolding

The session created deterministic PNG fixtures under `visual-tests/`.

Final source fixture geometry:

- outer image: `800x600`
- origin rectangle: `200x150` at `(0,0)`
- floating rectangle: `100x75` at `(460,300)`
- all rectangles use `4:3` ratio

Expected crop cases were generated for combinations of widget size, offset, and
scale, including the corrected scalar-2 purple offsets of `(920,600)` because
offset is in scaled-image pixels.

Scripts added under `visual-tests/scripts/`:

- `image_box_png.py`
- `generate_image_box_fixture_outputs.py`
- `test_widget_box_export_contract.py`

The saved PNG artifacts were moved into gitignored
`visual-tests/archived/pngs`, leaving the intended future test shape as a
contract test that is skipped until a real `tui-kit` widget-box export hook
exists.

### 3. `tui-kit` ImageViewport Interface

The session introduced or evolved a new viewport abstraction:

- `ViewportImage`
- `ImageViewport`
- `ImageViewportWidget`
- `ImageScale`
- `ZoomFactor`
- typed axes/directions for movement and zoom
- `ResizePolicy`, including `PreserveCenter`
- `ImageViewportInitialScale::{Native, FitToBox}`
- `ImageViewportOptions`
- `ImageViewportPlacement`

The interface supports:

- setting scale directly
- applying zoom by factor
- setting scaled offset
- setting unscaled/source offset
- stepping along typed axes/directions
- center-anchored zoom
- updating canvas metrics on resize

`c4tui` was then updated to store `ImageViewportWidget`s per view and route
pan, zoom, reset, hit-testing, and source placement through this model.

Verification reported at this stage:

- `tui-kit`: `97 passed`
- `c4tui`: `71 passed`
- Python export contract: skipped as expected

### 4. Build-Time Version/Stamp Work

The user wanted visible build-time stamps in the `c4tui` header to confirm that
new local builds were being used.

Changes made:

- `tui-kit/build.rs` sets `TUI_KIT_BUILD_TIME_HHMM`
- `tui-kit/src/lib.rs` exposes `BUILD_TIME_HHMM`
- `c4tui/build.rs` sets `C4TUI_BUILD_TIME_HHMM`
- `c4tui` header renders `c4tui [HH:mm] | [HH:mm]`

Verification:

- `cargo test` in both crates passed

The agent noted that this changed source/build behavior but did not install the
binary.

### 5. Reinstall Script for Local Development

The user asked for a script that cleans both crates, builds `tui-kit`, installs
`tui-kit`, builds `c4tui`, then installs `c4tui`.

Because `tui-kit` is a library crate, the script treats the `tui-kit` install
step explicitly as a build/verification step and then installs `c4tui`.

Script added in the `c4tui` repo:

- `/Users/coleshaffer/Projects/c4tui/scripts/reinstall-local.sh`

It was syntax-checked with `bash -n`; it was not run because it would clean both
repos and install the binary.

### 6. Diagnosis of Remaining `+` Zoom Bug

The user reported that pressing `+` still behaved incorrectly: the apparent
aspect ratio was stable only until one side hit the bounding widget, after
which further zooms changed only the other side or stopped having visible
effect.

The agent traced the `+` path:

- `c4tui/src/keymap.rs`
- `c4tui/src/event.rs`
- `c4tui/src/state.rs`
- `c4tui/src/view.rs`
- `tui-kit/src/widgets/image_viewport.rs`

Conclusion:

- The key path was using the new viewport API.
- The likely bug was terminal placement: placement dimensions were derived from
  `crop.visible.width/height`, causing the Kitty destination rectangle to grow
  per axis instead of keeping the widget box as a stable crop aperture.

### 7. Boundary Rework: `c4tui` Stops Building Raw Kitty Placement

After discussion, the agreed boundary became:

- `c4tui` owns app layout and says: first cell `(x,y)`, size `(cols, rows)`.
- `tui-kit` owns cell-pixel conversion, crop math, source rectangle, destination
  cells, image upload, Kitty placement, and resize updates.

Final implementation in this session:

In `tui-kit`:

- added `CellArea`
- added `Terminal::render_image_viewport`
- added `Terminal::render_viewport_image`
- added typed viewport init options
- changed Kitty placement to delete an existing placement id before re-placing

In `c4tui`:

- main rendering now passes only `CellArea` into `tui-kit`
- picker thumbnails now use the same `tui-kit` viewport render path
- picker thumbnails were rearranged into a grid
- thumbnails use `ImageViewportInitialScale::FitToBox`

Final verification reported:

- `cargo test` in `tui-kit`: `98 tests` passed
- `cargo test` in `c4tui`: `71 tests` passed

The agent explicitly did not reinstall the `c4tui` binary at the end.

## Important Files Touched or Discussed

In `tui-kit`:

- `src/widgets/image_box.rs`
- `src/widgets/image_viewport.rs`
- `src/widgets/mod.rs`
- `src/layout.rs`
- `src/image.rs`
- `src/terminal.rs`
- `src/lib.rs`
- `src/prelude.rs`
- `build.rs`
- `visual-tests/`

In `c4tui`:

- `src/view.rs`
- `src/terminal.rs`
- `src/picker.rs`
- `src/keymap.rs`
- `src/event.rs`
- `src/state.rs`
- `src/capabilities.rs`
- `build.rs`
- `scripts/reinstall-local.sh`

## Final State and Risks

The session ended with tests passing in both repos, but with uncommitted work.
The `c4tui` binary was not reinstalled after the final changes.

The main architectural improvement was moving image viewport terminal rendering
toward `tui-kit` ownership. The remaining conceptual risk is whether the final
Kitty placement behavior fully preserves the stable crop-aperture model in live
terminal rendering, especially across resize and non-square cell-pixel ratios.

The session also left a deliberate future test hook: compare actual exported
widget-box pixels from `tui-kit` against the generated pixel fixtures once an
export function exists.
