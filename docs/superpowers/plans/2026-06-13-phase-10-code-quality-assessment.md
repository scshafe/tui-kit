# Phase 10 - Code Quality Assessment and Refactoring Strategy

**Status:** COMPLETE - assessment only, 2026-06-13
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` Phase 10.
**Predecessors:** Phases 7, 8, and 9 are implemented and committed.

## Goal

Review both repos after the documentation, renderer-backend, and remote-launcher
work landed. This phase does not implement broad refactors. It identifies the
highest-value quality problems, orders them by impact, and converts them into
small follow-on remediation waves with explicit risks and verification strategy.

## Assessment Steps

- [x] Confirm the Phase 10 scope in the active roadmap.
- [x] Check both repo statuses and preserve the untracked
  `/Users/coleshaffer/Projects/tui-kit/visual-tests/` directory untouched.
- [x] Scan oversized modules, allowance markers, direct IO boundaries, renderer
  contracts, remote-prototype code, and c4tui orchestration modules.
- [x] Capture exact file/line references for each finding.
- [x] Produce follow-on phases sequenced by risk and dependency.

## Module Size Signals

The line counts below are not findings by themselves, but they identify where
review effort should concentrate.

### tui-kit

- `src/elements/mod.rs`: 3662 lines.
- `src/widgets/image_viewport.rs`: 1410 lines.
- `src/layout.rs`: 1217 lines.
- `src/widgets/grid.rs`: 1047 lines.
- `src/testkit.rs`: 811 lines.
- `src/image.rs`: 806 lines.
- `src/scheduler.rs`: 740 lines.
- `src/remote.rs`: 313 lines.
- `src/renderer.rs`: 109 lines.

### c4tui

- `src/app.rs`: 1457 lines.
- `src/workspace.rs`: 1020 lines.
- `src/state.rs`: 794 lines.
- `src/nav_picker.rs`: 771 lines.
- `src/viewport_state.rs`: 599 lines.
- `src/terminal.rs`: 556 lines.
- `src/statusbar.rs`: 547 lines.
- `src/render.rs`: 481 lines.
- `src/link_directory.rs`: 433 lines.
- `src/config.rs`: 421 lines.

## Findings

### P1 - c4tui `App` Still Owns Too Many Runtime Boundaries

**Impact:** High. `App` is the main user-facing workflow coordinator, so its
size and mixed responsibilities increase regression risk for modal navigation,
render scheduling, reload handling, log/help dialogs, and terminal cleanup.

**Evidence:**

- `c4tui/src/app.rs:46` defines `App` with catalog, render cache, viewport,
  state, workspace source, config, keymap, scheduler, focus, modal state, logs,
  and clipboard in one owner.
- `c4tui/src/app.rs:177` runs the main event loop and directly calls rendering
  after startup.
- `c4tui/src/app.rs:328` implements modal-key handling, hover-render requests,
  modal close, command application, and full rerendering in one method.
- `c4tui/src/app.rs:532` begins `Effect::OpenModal` handling inline inside the
  command/effect path.
- `c4tui/src/app.rs:541`, `c4tui/src/app.rs:613`, and
  `c4tui/src/app.rs:678` repeat modal construction, initial hover calculation,
  render-cache priming, focus-scope push, active-modal assignment, and
  `terminal.render_modal` calls for view, child-view, and link-directory modals.
- `c4tui/src/app.rs:738` handles reload, image-cache clearing, log toggling,
  help dialogs, and redraw policy in the same effect dispatcher.
- `c4tui/src/app.rs:890` starts a large inline test module, keeping extensive
  fixture setup in the same file as production orchestration.

**Assessment:** The Phase 3-6 work gave c4tui better concepts
(`ActiveModal`, `ViewCatalog`, `RenderCache`, `ViewportState`, `LinkDirectory`),
but `App` still composes those concepts with repeated branch-heavy code. The
problem is not the existence of one application coordinator; it is that modal
construction, effect execution, scheduler interaction, and rendering policy are
not isolated enough to test or change independently.

**Recommended direction:** Extract a small modal factory first, then an
`EffectRunner` or equivalent command/effect executor that receives a narrow
`AppRuntime` context. Keep `App` as the event-loop owner, but move branch-heavy
construction and post-effect render decisions behind focused helpers.

### P1 - tui-kit `elements` Is a Public-Surface Monolith

**Impact:** High. `elements` is retained intentionally for the renderer/effect
direction, but the current single-file shape makes public API review,
effect-lifecycle auditing, and focused test maintenance too expensive.

**Evidence:**

- `tui-kit/src/elements/mod.rs:1` documents the full declarative element layer.
- `tui-kit/src/elements/mod.rs:27` through `tui-kit/src/elements/mod.rs:40`
  define the public `ElementOutcome` alias and `Element` marker trait.
- `tui-kit/src/elements/mod.rs:42` starts container traits in the same file as
  decorators, windows, modals, overlays, image elements, and tests.
- `tui-kit/src/elements/mod.rs:2116` defines `ImageViewportElement`, an
  effect-aware image wrapper inside the same module as generic element traits.
- `tui-kit/src/elements/mod.rs:2200` emits image render effects and clears
  dirty state in that wrapper.
- `tui-kit/src/elements/mod.rs:2240` starts the modal wrapper after the image
  effect implementation.
- `tui-kit/src/elements/mod.rs:2556` starts a large inline test module that
  exercises many unrelated element responsibilities.

**Assessment:** The Phase 7 retention audit documents why these APIs remain,
and Phase 8 adds a real renderer backend contract. That makes the module more
important, not less. Keeping every primitive, decorator, window, modal, overlay,
effect wrapper, and test in one module creates a public API review bottleneck and
makes it easy for unrelated changes to collide.

**Recommended direction:** Split `elements` into internal modules such as
`core`, `decorators`, `containers`, `window`, `modal`, `overlay`, `image`, and
`effect`, with the existing public names re-exported from `elements::mod`. This
is a structural split, not a naming compatibility layer; public names that
remain should be the final names.

### P2 - Remote Launcher Needs a Transport and Session State Boundary

**Impact:** Medium-high. Phase 9 correctly landed a prototype, but expanding it
without isolating process IO and protocol state would turn the prototype into a
hard-to-test runtime.

**Evidence:**

- `tui-kit/src/remote.rs:1` explicitly says the module is a prototype and not a
  stable wire protocol.
- `tui-kit/src/remote.rs:21` through `tui-kit/src/remote.rs:24` define the
  handshake environment and CLI usage.
- `tui-kit/src/remote.rs:60` builds direct SSH arguments with `-tt`, `env`, and
  the remote command.
- `tui-kit/src/remote.rs:164` defines `RemoteFrameDecoder` with only a pending
  line buffer.
- `tui-kit/src/remote.rs:178` treats non-frame lines as passthrough whenever no
  frame is pending.
- `tui-kit/src/remote.rs:199` decodes `TKF1` frames directly into
  `RenderFrame`.
- `tui-kit/src/remote.rs:257` parses effect strings into `RenderEffect`.
- `tui-kit/src/bin/tui-kit-cli.rs:28` spawns `ssh` directly.
- `tui-kit/src/bin/tui-kit-cli.rs:45` enters the local terminal before the
  remote stream has proven it can emit frames.
- `tui-kit/src/bin/tui-kit-cli.rs:48` starts an input writer thread whose result
  is not joined.
- `tui-kit/src/bin/tui-kit-cli.rs:63` drives decoder output and local rendering
  in the same loop.

**Assessment:** The current code is acceptable as a prototype because Phase 9
kept the protocol minimal and heavily unit tested. The next remote wave should
avoid adding host profiles, reconnection behavior, richer frame encoding, or
remote app helpers until subprocess lifecycle and protocol state are represented
as first-class types.

**Recommended direction:** Add a `RemoteTransport` trait and a `RemoteSession`
state machine. Model startup, first-frame/handshake, frame streaming, passthrough
policy, EOF, and child exit explicitly. Keep `ssh` as one implementation, then
test the session with in-memory transport fixtures.

### P2 - c4tui Workspace Loading Mixes Export, JSON Parsing, and Indexing

**Impact:** Medium. Workspace behavior is product-critical and reasonably well
tested, but the module combines process execution, source resolution,
Structurizr JSON parsing, metadata enrichment, view discovery, relationship
indexing, and tests.

**Evidence:**

- `c4tui/src/workspace.rs:10` starts the workspace source and exported-workspace
  types.
- `c4tui/src/workspace.rs:22` defines `WorkspaceModel` and relationship indexes.
- `c4tui/src/workspace.rs:225` runs the external `structurizr` CLI.
- `c4tui/src/workspace.rs:268` loads and parses the exported workspace JSON,
  returning default data on missing or invalid model input.
- `c4tui/src/workspace.rs:280` through `c4tui/src/workspace.rs:296` traverses
  people, systems, containers, components, and deployment nodes.
- `c4tui/src/workspace.rs:531` starts view metadata matching and `ViewInfo`
  assembly.
- `c4tui/src/workspace.rs:565` and `c4tui/src/workspace.rs:580` wire key views
  and child views.
- `c4tui/src/workspace.rs:856` starts resolver and parser tests in the same
  file.

**Assessment:** The module has multiple clean internal concepts already. They
should become module boundaries before adding more workspace formats, discovery
rules, or diagnostics. The default-on-parse-failure path is a deliberate product
choice today, but it should be isolated so future diagnostics can distinguish
"no metadata" from "metadata exists but is malformed."

**Recommended direction:** Split into `workspace/source.rs`, `workspace/export.rs`,
`workspace/model.rs`, `workspace/metadata.rs`, and `workspace/discover.rs`. Keep
the public loader API narrow, and add fixture-backed tests for malformed JSON,
missing metadata, key-view matching, child-view wiring, and relationship indexes.

### P2 - Image Geometry Ownership Is Split Across Two Large Modules

**Impact:** Medium. The image viewport and placement math are heavily tested and
central to c4tui's user experience. The current ownership boundary is mostly
working, but it leaves two large modules with overlapping concepts and some
allowance markers.

**Evidence:**

- `tui-kit/src/layout.rs:1` has a crate-level `#![allow(dead_code)]`.
- `tui-kit/src/layout.rs:3` documents deterministic image layout math,
  including fit, zoom, pan, clipping, placement, and tail-following viewport
  calculations.
- `tui-kit/src/layout.rs:208` defines the public `Placement` output model.
- `tui-kit/src/layout.rs:324` defines `PlacementPolicy`.
- `tui-kit/src/layout.rs:500` gives `ViewTransform` convenience methods that
  construct a built-in `PlacementEngine`.
- `tui-kit/src/widgets/image_viewport.rs:77` defines `ImageViewport` with image
  scale, offset, step, and zoom state.
- `tui-kit/src/widgets/image_viewport.rs:351` defines `ImageViewportWidget`
  with viewport, canvas, and resize policy.
- `tui-kit/src/widgets/image_viewport.rs:410` updates canvas geometry and applies
  resize policy.
- `tui-kit/src/widgets/image_viewport.rs:490` exposes widget placement by
  delegating through viewport state.
- `tui-kit/src/widgets/image_viewport.rs:970` shows detailed pixel-crop tests in
  the same module.
- `tui-kit/src/widgets/image_viewport.rs:909` and `tui-kit/src/layout.rs:782`
  require `clippy::too_many_arguments` allowances.

**Assessment:** `layout` should own pure geometry policy and deterministic
placement. `ImageViewport` should own interactive viewport state and image
export behavior. The boundary is close, but the built-in convenience paths and
duplicated vocabulary make it harder to tell where new pan, zoom, fit, or crop
behavior belongs.

**Recommended direction:** Keep behavior stable, but refactor around a sharper
contract: `layout` exposes pure policy/placement builders and golden-case
helpers; `image_viewport` owns state transitions and raster export. Remove or
narrow the `dead_code` allowance as the public retention audit becomes the
source of truth.

### P2 - Terminal Side Effects Still Bypass the Renderer Abstraction

**Impact:** Medium. Phase 8 introduced `RenderFrame` and `RendererBackend`, but
some low-level IO still writes directly to stdout or raw terminal state. That
limits deterministic tests for image escape sequencing and capability probing.

**Evidence:**

- `tui-kit/src/renderer.rs:23` defines `RenderFrame`.
- `tui-kit/src/renderer.rs:58` defines the `RendererBackend` trait.
- `tui-kit/src/renderer.rs:70` defines `ImageSurfaceRenderer` for image-surface
  backed tests and non-terminal paths.
- `tui-kit/src/terminal.rs:116` enters raw mode and alternate screen.
- `tui-kit/src/terminal.rs:153` renders a `RenderFrame` locally, then applies
  effects through the image registry.
- `tui-kit/src/terminal.rs:251` restores terminal state in `Drop`.
- `tui-kit/src/image.rs:414`, `tui-kit/src/image.rs:423`,
  `tui-kit/src/image.rs:451`, `tui-kit/src/image.rs:466`, and
  `tui-kit/src/image.rs:476` write Kitty placement, deletion, cursor movement,
  and image transmission escapes directly to `io::stdout`.
- `c4tui/src/capabilities.rs:118` starts a raw terminal probe session.
- `c4tui/src/capabilities.rs:141` and `c4tui/src/capabilities.rs:152` write
  Kitty graphics and SGR pixel-mouse probes directly to stdout.

**Assessment:** Direct terminal writes are appropriate at the hardware boundary,
but the boundary is not narrow enough. `ImageSurfaceRegistry` and capability
probes need injectable writers/readers so escape generation and cleanup behavior
can be tested without a live terminal.

**Recommended direction:** Add internal IO traits or generic writer wrappers for
Kitty image output and capability probes. Keep public terminal APIs simple; move
testability into internal constructors and `testkit` helpers.

### P3 - Public Retention Discipline Still Relies on Broad Allowances

**Impact:** Medium-low. Phase 7 added retention documentation and audit tooling,
but broad dead-code allowances can hide stale surfaces and delay cleanup.

**Evidence:**

- `tui-kit/src/layout.rs:1` has `#![allow(dead_code)]`.
- `c4tui/src/log_view.rs:1`, `c4tui/src/clipboard.rs:1`,
  `c4tui/src/nav_picker.rs:13`, `c4tui/src/statusbar.rs:1`,
  `c4tui/src/nav_items.rs:15`, `c4tui/src/keymap.rs:1`, and
  `c4tui/src/logger.rs:1` have module-level `#![allow(dead_code)]`.
- `c4tui/src/app.rs:171`, `c4tui/src/main.rs:135`,
  `c4tui/src/terminal.rs:52`, and `c4tui/src/terminal.rs:57` have local
  `allow(dead_code)` markers.

**Assessment:** Some allowances are temporary pressure relief from the rapid
refactor sequence, and some may be justified by product features behind future
paths. They should not persist as broad module defaults. Public surface retention
should be explained by consumers, tests, or specific documented reasons, not by
blanket lints.

**Recommended direction:** Remove broad allowances in small batches. When code
is retained, add a test, c4tui consumer, or narrow documentation entry. When it
is not retained, delete it outright per the roadmap's no-shim rule.

### P3 - Inline Tests and Fixtures Obscure Production Code

**Impact:** Low-medium. The test coverage is valuable, but large inline test
modules make production files harder to review and duplicate fixture setup.

**Evidence:**

- `tui-kit/src/elements/mod.rs:2556` starts an inline element test module after
  more than 2500 lines of production definitions.
- `c4tui/src/app.rs:890` starts inline app tests with SVG fixture creation at
  `c4tui/src/app.rs:912`.
- `c4tui/src/workspace.rs:856` starts inline workspace resolver and parser
  tests.
- `tui-kit/src/testkit.rs:1` already positions the crate as the home for widget,
  event-stream, scheduler, and image-side-effect test helpers.
- `tui-kit/src/testkit.rs:50` and `tui-kit/src/testkit.rs:86` provide reusable
  buffer/effect helpers that should be leaned on as more tests move out of
  production modules.

**Assessment:** Inline tests are acceptable for private helpers, but the largest
production files now carry enough fixture code that review signal suffers. Moving
tests to focused integration modules will likely require a few `pub(crate)` test
builders, which is acceptable if it reduces production-file size and improves
fixture reuse.

**Recommended direction:** Move large behavioral suites into `tests/` or
dedicated `*_tests.rs` modules where privacy allows. Promote repeated SVG,
workspace, modal, and render-effect fixtures into local test support modules or
`tui_kit::testkit` when they are library-generic.

## Remediation Waves

### Phase 11 - c4tui App Orchestration and Modal Factory

**Scope:** Extract modal construction and post-command effect execution from
`App` without changing user-visible behavior.

**Proposed work:**

- Add a `modal_factory` module that constructs view, child-view, and link
  modals from `ViewCatalog`, `AppState`, and render-cache status.
- Centralize focus-scope push and initial hover render priming.
- Add an effect executor that handles reload, cache clearing, help/log dialogs,
  and rerender policy through a small context object.
- Keep `App::run` as the event-loop owner.

**Risks:** Modal focus regressions, missed thumbnail pre-render requests, stale
image teardown, and subtle differences in render timing.

**Test strategy:** Run the existing c4tui app tests; add factory-level tests for
each modal spec; assert terminal calls with `FakeTerminalBackend`; add targeted
tests for hover render request priming and focus-scope cleanup.

### Phase 12 - tui-kit Elements Module Split

**Scope:** Split `elements/mod.rs` into focused modules while preserving final
public names and current behavior.

**Proposed work:**

- Split core traits, decorators, containers, window/modal, overlay, image, and
  effect code into separate files.
- Keep public re-exports in `elements::mod` only for names that remain part of
  the final API.
- Move large tests to module-specific tests and keep render-effect coverage.

**Risks:** Public API drift, missed re-exports, dirty-state regressions, effect
teardown regressions, and test privacy friction.

**Test strategy:** Run `cargo test`, `cargo doc --no-deps`, and the public
surface audit. Add explicit tests for `ImageViewportElement` placement and
teardown effects after the split.

### Phase 13 - Remote Transport and Session State Machine

**Scope:** Turn the Phase 9 prototype into a testable remote runtime boundary
before adding new remote features.

**Proposed work:**

- Introduce `RemoteTransport` for input writes, output reads, process exit, and
  stderr policy.
- Introduce `RemoteSession` with states for startup, awaiting first frame,
  streaming, passthrough, EOF, and child exit.
- Move SSH process construction behind one transport implementation.
- Keep the `TKF1` decoder until the state boundary is stable.

**Risks:** Terminal lifecycle ordering, blocked writer threads, dropped input
events, EOF mid-frame behavior, and noisy remote stdout corrupting local UI.

**Test strategy:** Add in-memory transport tests for malformed frames, EOF while
pending, passthrough before first frame, non-zero child exit, and resize/input
forwarding. Keep live SSH/manual testing as operator-owned until a remote fixture
exists.

### Phase 14 - c4tui Workspace Module Split

**Scope:** Split workspace source resolution, export execution, JSON parsing,
metadata enrichment, model indexing, and view discovery.

**Proposed work:**

- Create `workspace/source.rs`, `workspace/export.rs`, `workspace/model.rs`,
  `workspace/metadata.rs`, and `workspace/discover.rs`.
- Keep a narrow public facade from `workspace/mod.rs`.
- Preserve current fallback behavior while exposing typed diagnostics internally.

**Risks:** Structurizr export path changes, key-view matching regressions,
metadata parse fallback changes, and relationship index drift.

**Test strategy:** Move existing workspace tests with fixtures; add malformed
metadata and missing-json tests; assert relationship indexes and child/key-view
wiring with deterministic fixture JSON.

### Phase 15 - Image Geometry Boundary Cleanup

**Scope:** Clarify ownership between pure layout geometry and interactive image
viewport state.

**Proposed work:**

- Move pure placement policy validation and builders into `layout`.
- Keep viewport state transitions, raster export, and widget resize policy in
  `image_viewport`.
- Replace broad allowances with narrow code or retention docs.
- Add golden placement cases shared by `layout` and `image_viewport` tests.

**Risks:** Pixel rounding changes, zoom anchor drift, crop/export regressions,
and c4tui viewport navigation behavior changes.

**Test strategy:** Preserve existing viewport pixel tests; add golden
placement-policy cases; run c4tui app tests that exercise zoom, pan, resize, and
link navigation.

### Phase 16 - Terminal IO and Capability Probe Test Boundaries

**Scope:** Make terminal escape output and capability probes testable without a
live terminal.

**Proposed work:**

- Add internal writer abstractions for Kitty image output.
- Add injectable read/write handles for capability probes.
- Keep public terminal construction simple and production-first.
- Extend `testkit` with escape capture helpers only if they remain
  library-generic.

**Risks:** Terminal cleanup regressions, cursor placement differences, Kitty
chunking mistakes, and capability false positives/negatives.

**Test strategy:** Add unit tests for generated Kitty upload/place/delete
escapes, chunking, cleanup, and probe parsing. Keep the existing image smoke
checklist operator-owned for live terminal validation.

### Phase 17 - Public API and Test Fixture Hygiene

**Scope:** Remove stale allowances, move large tests out of production files,
and strengthen retention documentation.

**Proposed work:**

- Remove `dead_code` allowances in batches.
- Delete unused surfaces instead of preserving migration names.
- Move large inline test suites to focused test modules.
- Promote reusable fixtures into c4tui test support or `tui_kit::testkit` where
  they are genuinely reusable across crates.

**Risks:** Accidental deletion of planned-but-real API, privacy churn from test
movement, and over-generalized test helpers.

**Test strategy:** Run both repos' fmt, clippy, and tests after each batch; run
the tui-kit public-surface audit; prefer local fixture helpers until reuse is
proven.

## Follow-On Phase List

1. **Phase 11 - c4tui App Orchestration and Modal Factory**
   - Reduces the highest user-facing complexity first.
   - Unblocks cleaner log/help/reload effect handling.

2. **Phase 12 - tui-kit Elements Module Split**
   - Makes the retained render/effect element layer reviewable.
   - Reduces risk before more remote-render work depends on it.

3. **Phase 13 - Remote Transport and Session State Machine**
   - Hardens the Phase 9 prototype before adding host profiles or richer wire
     protocol features.

4. **Phase 14 - c4tui Workspace Module Split**
   - Isolates product data loading and indexing after app orchestration is
     simpler.

5. **Phase 15 - Image Geometry Boundary Cleanup**
   - Clarifies math ownership after the element and workspace splits reduce
     surrounding churn.

6. **Phase 16 - Terminal IO and Capability Probe Test Boundaries**
   - Narrows the real-terminal boundary and improves deterministic testing.

7. **Phase 17 - Public API and Test Fixture Hygiene**
   - Sweeps allowances and test organization after the main structural moves
     have created better homes for retained code.

## Verification for Phase 10

Phase 10 changed documentation only. Required verification is:

- `git diff --check`
- confirm no files under `/Users/coleshaffer/Projects/tui-kit/visual-tests/`
  were edited or staged
- commit this assessment document
