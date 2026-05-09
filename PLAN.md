# tui-kit — AI-First Roadmap and Interface Plan

> **Feedback-loop correction (supersedes earlier sections of this document).**
>
> This document was the source of a speculative-build pattern: most of its
> ambitions were implemented as primitives without consumer pressure. The
> codebase has since been demolished back to the API floor that c4tui actually
> consumes. See `PLAN_REWRITE.md` for the current discipline.
>
> **The new rule:** before adding a public module to tui-kit, name the consumer
> that will use it within this commit (or the immediately following commit on
> the c4tui side). If the answer is "the dashboard might want it" or "this is
> obviously useful," that is the speculative pattern again. Stop, and add the
> consumer first.
>
> **Surfaces currently on probation:** `component`/`Cached` and modal focus are
> now consumed by c4tui and should keep evolving through that feedback loop. A
> future tick producer should only re-enter with c4tui heartbeat demand. Generic
> focus traversal was pruned and should re-enter only with a named consumer.
>
> **Modules deliberately removed** (will re-enter only with named consumers):
> `widgets::list`, `widgets::table`, `widgets::tree`, `widgets::tabs`, `theme`,
> `runtime`, `subscription`, `widgets::picker`, `ImageConfig`, `WatcherConfig`,
> `SchedulerConfig`, generic focus traversal.
>
> **Process gates** (see `.github/workflows/ci.yml`):
> - `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `cargo doc --no-deps`
>   on tui-kit
> - `cargo check && cargo test` on c4tui against the local tui-kit
>
> If the consumer-gate is red, the tui-kit change does not land. This replaces
> "API will change" as the discipline.
>
> Read the rest of this document as historical roadmap notes, not as a backlog.

---

This file is the working product/API plan for `tui-kit`. It intentionally prioritizes library power, explicit behavior, flexible mechanics, and machine/agent readability over near-term refactor effort or human-readable minimalism.

## North star

`tui-kit` should be a reusable Rust substrate for sophisticated terminal applications that need:

- unified event delivery;
- reusable, policy-light UI state machines;
- ratatui-native rendering without hiding ratatui;
- terminal image lifecycle coordination, initially optimized for Kitty/WezTerm;
- scheduling, cancellation, progress, and wakeups for expensive work;
- explicit configuration with noisy validation and few/no silent defaults;
- component, focus, layout, theme, test, and runtime primitives that make complex TUIs easier to assemble.

`tui-kit` owns mechanics, lifecycle, coordination, and reusable state machines. Applications own domain meaning, command semantics, workflow policy, and product-specific interpretation.

## Current baseline, as of this plan

The crate already provides:

- `events`: one concrete `AppEvent` channel for keys, resize, scheduler completion, workspace changes, heartbeat.
- `input` / `input_thread`: crossterm input conversion and producer thread.
- `keymap`: declarative key bindings.
- `tty`: terminal cell/pixel metrics.
- `image`: `ImageSurface` trait plus `KittyImageRegistry`.
- `layout`: image placement, fit, pan, zoom math.
- `bar`: slot-aligned status bar registry.
- `scheduler`: thread-backed priority scheduler with dedupe, epoch invalidation, progress, completions.
- `watcher`: notify/debounce producer.
- `widgets::picker`: generic item picker with filtering, groups, thumbnails.
- `widgets::dialog`: modal dialog.
- `terminal`: raw-mode/alt-screen terminal wrapper with image registry.

The current implementation is useful, but several APIs still contain app-shaped assumptions or under-specified behavior. This plan treats those as design pressure, not as blockers caused by refactor cost.

## Design laws

1. **No app policy in toolkit mechanics.** Widgets may own navigation, filtering, selection, visibility state, layout, and rendering. They must not assign domain meaning like “Tab toggles the hidden legend group.”
2. **One event channel remains a core opinion.** Input, resize, ticks, scheduler completions, watcher updates, and app-defined events should flow through one dispatch point.
3. **One scheduler remains a core opinion.** Expensive work should share dedupe, priority, cancellation, instrumentation, and completion wakeup behavior.
4. **One image surface/registry abstraction remains a core opinion.** Terminal image protocols are subtle; lifecycle and cleanup should be centralized.
5. **Avoid hiding ratatui.** Expose `Frame`, `Rect`, `Buffer`, `Widget`, styles, and placement information directly. `tui-kit` should complement ratatui, not replace it.
6. **Policy-light widgets.** Widgets implement reusable UI mechanics. Apps map keys and outcomes into commands and domain behavior.
7. **Explicit configuration over silent defaults.** Defaults may be easy to install up front, but ambiguous/missing/misconfigured behavior should validate loudly and preferably fail early.
8. **Machine-readable APIs and docs first.** Prefer explicit enum variants, structured config, validation errors, stable IDs, exhaustive docs on ownership/extension points, and examples that agents can copy mechanically.
9. **WezTerm/Kitty is the immediate runtime target.** Keep abstractions clean enough for future backends, but do not let broad terminal portability slow the WezTerm/Kitty path.
10. **Refactor effort is not a constraint.** If a better architecture enables the target functionality, choose it.

## Configuration philosophy

The library should strongly avoid silent defaults in operational paths.

Recommended shape:

```rust
pub trait Validate {
    fn validate(&self) -> Result<(), ConfigError>;
}

pub trait KitConfig: Validate + Clone + std::fmt::Debug {}
```

Every major subsystem should expose an explicit config struct and validation pass:

- `RuntimeConfig`
- `TerminalConfig`
- `ImageConfig`
- `PlacementConfig`
- `PickerConfig`
- `ListConfig`
- `TableConfig`
- `TreeConfig`
- `FocusConfig`
- `SchedulerConfig`
- `ThemeConfig`
- `TickConfig`
- `TestHarnessConfig`

Defaults should exist only as named constructors/presets, not invisible behavior:

```rust
let config = RuntimeConfig::strict_wezterm_kitty();
let config = RuntimeConfig::headless_test();
let config = RuntimeConfig::degraded_no_images();
```

Prefer:

- `RuntimeConfig::strict_wezterm_kitty()` over `Default::default()` in examples.
- `try_build()` builders that return `Result<_, ConfigError>`.
- warnings/errors for unset key policy, unset image overflow policy, ambiguous focus traversal, unsupported image protocol, impossible layout constraints, and missing terminal metrics.
- `#[non_exhaustive]` on public enums likely to grow.

Open decision: whether to ban `Default` on public runtime config entirely, or implement `Default` only for inert/test structs where no runtime policy is implied. Recommendation: avoid `Default` for behavior-bearing config.

## Milestone 1 — Picker API cleanup

### Problem

`Picker::handle_key` currently maps `Tab` to `PickerOutcome::ToggleHiddenGroup("default")`. This is app policy embedded in a generic widget.

### Direction

Replace hardcoded key handling with configurable actions.

Proposed API:

```rust
pub enum PickerAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Select,
    Cancel,
    ClearFilterOrCancel,
    AppendFilterChar(char),
    BackspaceFilter,
    ToggleGroup { group: String },
    ToggleSelectedItemGroup,
    Custom(String),
}

pub struct PickerConfig {
    pub title: String,
    pub bottom_hint: String,
    pub thumb_cols: u16,
    pub thumb_rows: u16,
    pub item_row_span: u16,
    pub allow_thumbnails: bool,
    pub actions: KeyMap<PickerAction>,
    pub validation: PickerValidationPolicy,
}
```

Alternative, smaller interim shape:

```rust
pub struct PickerConfig {
    pub toggle_group: Option<String>,
    // existing fields...
}
```

Recommendation: implement `KeyMap<PickerAction>` rather than a single `toggle_group` escape hatch. This keeps the widget generic and future-proofs navigation policy.

### Requirements

- `PickerItem<T>` remains generic: `id`, `primary`, `detail`, `group`, `searchable`, `payload`.
- No domain concepts enter picker items.
- `PickerOutcome` should report generic mechanics only: select ID/payload, cancel, action/custom action, visibility changed.
- Config validation should error if a key is bound to an action requiring missing config.
- Existing c4tui-specific group/legend behavior moves into c4tui key/action mapping.

## Milestone 2 — Explicit image placement and zoom policy

### Problem

Image zoom behavior is currently encoded in `ViewTransform::place`: fit-scale first, then zoom, crop to canvas when overfit, center in available cells. That is a valid policy, but applications need to choose between at least these behaviors:

- scale image down/up to stay inside a region;
- allow overflow/cutoff while continuing to zoom unscaled;
- prevent further zoom once a boundary is reached;
- crop source versus crop destination;
- preserve anchor under cursor versus recenter;
- choose how thumbnails and main images behave differently.

### Direction

Promote image placement behavior into explicit configuration.

Proposed API:

```rust
pub enum ImageZoomLimitPolicy {
    ClampScale { min: f32, max: f32 },
    ClampAtFitBounds,
    AllowUnboundedWithinProtocol,
}

pub enum ImageOverflowPolicy {
    FitWithinArea,
    CropSourceToArea,
    OverflowAndClipDestination,
    PreventZoomBeyondArea,
}

pub enum ImageScaleBasis {
    NativePixels,
    FitToArea,
    FillArea,
    ExplicitScale(f32),
}

pub enum ImageAnchorPolicy {
    Center,
    PreserveCursorAnchor,
    PreserveImagePoint { x: f32, y: f32 },
}

pub struct PlacementPolicy {
    pub scale_basis: ImageScaleBasis,
    pub zoom_limit: ImageZoomLimitPolicy,
    pub overflow: ImageOverflowPolicy,
    pub anchor: ImageAnchorPolicy,
    pub min_visible_pixels: PixelSize,
    pub cell_rounding: CellRoundingPolicy,
}
```

`ViewTransform` should become pure state. Placement calculation should be:

```rust
let placement = PlacementEngine::new(policy).place(image, area, transform, metrics)?;
```

### Requirements

- Main views, thumbnails, previews, and charts can use different placement policies.
- Misconfigured placement policy errors loudly, especially contradictory overflow/zoom constraints.
- Docs must state invariants for coordinating ratatui cells and terminal image placements.
- Placement result should expose enough metadata for debugging: effective scale, fit scale, source rect, destination cells, clipped sides, anchor result.
- WezTerm/Kitty behavior is the initial correctness target.

## Milestone 3 — Image surface system

### Direction

Keep `ImageSurface` as the seam. Add a surface selection layer without slowing Kitty/WezTerm support.

Proposed types:

```rust
pub enum ImageBackendPreference {
    KittyOnly,
    AutoDetect { order: Vec<ImageProtocol> },
    Explicit(ImageProtocol),
    Disabled,
}

pub enum ImageProtocol {
    Kitty,
    Sixel,
    ITerm2,
    Noop,
}

pub struct ImageSurfaceRegistry { /* selected surface + capabilities */ }
```

### Near-term requirements

- Strong Kitty/WezTerm implementation.
- `NoopImageSurface` for image-disabled/degraded mode.
- `ImageCapabilities` describing protocol, max dimensions if known, placement support, deletion support, transparency assumptions.
- Runtime selection can initially be explicit config rather than clever auto-detection.

### Later requirements

- Sixel surface.
- iTerm2 image surface.
- Capability-driven selection/probing.
- Protocol-specific docs and test fixtures.

## Milestone 4 — Event architecture hardening

### Direction

Keep one channel, but prevent `AppEvent` from becoming a junk drawer.

Proposed API:

```rust
pub enum AppEvent<UserEvent = std::convert::Infallible> {
    Input(InputEvent),
    Terminal(TerminalEvent),
    Runtime(RuntimeEvent),
    Scheduler(SchedulerEvent),
    Watcher(WatcherEvent),
    Tick(TickEvent),
    User(UserEvent),
}
```

### Requirements

- Unified delivery, typed categories.
- Clear ownership docs for each event category.
- User events do not require forking the enum.
- Producers have names/IDs where useful.
- Events are structured enough for agents to inspect and route mechanically.

## Milestone 5 — Tick/timer producers

### Direction

Add ergonomic periodic producers that send through the unified event channel.

Proposed API:

```rust
pub struct TickSourceId(String);

pub struct TickConfig {
    pub id: TickSourceId,
    pub interval: Duration,
    pub start: TickStartPolicy,
    pub missed_tick_policy: MissedTickPolicy,
}
```

### Requirements

- Fixed interval producer.
- Named tick sources.
- Start immediately vs after first interval.
- Stop handle.
- Validation rejects zero/absurd intervals unless explicitly allowed for tests.
- No async runtime dependency required.

## Milestone 6 — Component trait, dirty tracking, and cached rendering

### Direction

Introduce a component abstraction for reusable, retained-ish UI without replacing ratatui.

Proposed API:

```rust
pub trait Component {
    type Event;
    type Message;

    fn id(&self) -> ComponentId;
    fn render(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) -> Result<()>;
    fn handle_event(&mut self, event: &Self::Event) -> Result<ComponentOutcome<Self::Message>>;
    fn dirty(&self) -> DirtyState;
    fn mark_dirty(&mut self, reason: DirtyReason);
    fn clear_dirty(&mut self);
    fn focus_node(&self) -> Option<FocusNode>;
    fn children(&self) -> ComponentChildren<'_>;
}
```

Add:

- `DirtyState`: clean, dirty, layout-dirty, paint-dirty, image-placement-dirty.
- `DirtyReason`: input, resize, data update, timer, theme, explicit.
- `Cached<C>` wrapper storing last rendered buffer/area/theme/input hash where safe.
- Invalidations that are explicit and inspectable.

### Requirements

- Expensive widgets can avoid full recomputation.
- Image placement invalidation is distinct from text buffer invalidation.
- Dirty behavior validates assumptions in debug/test mode.
- Components remain optional; apps can keep direct event loops and ratatui draws.

## Milestone 7 — Focus management

### Direction

Add generic focus traversal and modal capture.

Proposed types:

```rust
pub struct FocusManager;
pub struct FocusNode;
pub enum FocusScopeKind { Normal, Modal, Capturing }
pub enum FocusTraversal { Forward, Backward, Explicit(FocusId) }
```

### Requirements

- Tab/Shift+Tab traversal is configurable, not globally assumed.
- Modal focus capture and restoration.
- Focus stack for nested modals/dialogs/popovers.
- Stable IDs for focusable widgets.
- Clear event routing: focused component first, bubbling/capture policies explicit.

## Milestone 8 — Widget suite

### 8.1 Scrollable list

Separate plain list mechanics from picker mechanics.

Required features:

- selection optional;
- scrolling independent from selection;
- configurable wrapping/truncation;
- viewport math exposed/tested;
- key actions configurable;
- no filtering requirement.

### 8.2 Table

Operational tools need tables.

Required features:

- fixed headers;
- row selection;
- horizontal and vertical scrolling;
- column sizing policies: fixed, percentage, content, fill, min/max;
- truncation and alignment;
- sorting hooks but app-owned sort semantics;
- optional pinned columns;
- stable row IDs.

### 8.3 Tree

Hierarchical data is common: files, configs, metrics, services, traces.

Required features:

- expand/collapse mechanics;
- visible flattened projection;
- selection;
- lazy child loading hooks;
- stable node IDs;
- optional checkbox/tri-state mechanics without app semantics.

### 8.4 Tabs and panes

Required features:

- selected tab state;
- tab close/reorder hooks if apps need them;
- pane focus model;
- split pane sizing policies;
- resize constraints;
- layout result inspectable for tests/agents.

### 8.5 Existing widgets

- Picker becomes action-configured and policy-light.
- Dialog gains explicit focus/cancel/confirm policy.
- Segment bar gains theme integration and optional common segments.

## Milestone 9 — Scheduler evolution

### Direction

Keep the scheduler generic and policy-narrow.

### Requirements

- Execute work and report completion; no rendering semantics.
- Keep dedupe by ID.
- Add cancellation granularity:
  - by ID;
  - by group;
  - by source;
  - by epoch namespace;
  - all.
- Add queue instrumentation:
  - pending count;
  - active workers;
  - queued count;
  - completed/failed/cancelled totals;
  - timing stats;
  - oldest/newest queued age;
  - worker utilization if cheap.
- Consider generic priorities:

```rust
pub struct Scheduler<Item, Out, P = Priority>
where
    P: Ord + Clone + Send + 'static;
```

Keep current `Priority::{Background, Hover, Active}` as the default until pressure proves otherwise.

## Milestone 10 — Terminal/runtime shell

### Direction

Decide how much app-loop scaffolding belongs in `tui-kit` after patterns emerge, but terminal lifecycle should be owned by the toolkit.

### Requirements

- Raw mode lifecycle.
- Alternate screen lifecycle.
- Mouse capture configuration.
- Bracketed paste configuration if used.
- Terminal restoration on panic/drop where possible.
- Image cleanup/shutdown.
- Resize handling.
- Metrics refresh.
- Optional `AppShell` / `Runtime` that wires producers, terminal, scheduler, tick sources, and event dispatch.

Proposed runtime layering:

```rust
TerminalSession // owns terminal lifecycle
Runtime         // owns event producers and shared services
AppShell        // optional event loop scaffold
App             // app-owned command/domain behavior
```

Do not require apps to use `AppShell` if direct control is better.

## Milestone 11 — Theme and style primitives

### Direction

Add a small named-role theme system, not a giant design system.

Required roles:

- normal text;
- dim text;
- selection;
- selection inactive;
- border;
- focused border;
- warning;
- error;
- success;
- accent;
- background if supported;
- title/header/footer.

Requirements:

- Apps can override every style.
- Widgets do not hardcode visual policy except through explicit theme/config.
- Theme validation catches missing roles unless a preset explicitly fills them.
- Style roles are stable and documented.

## Milestone 12 — Data/update subscription primitives

### Direction

Add cautiously. Useful for dashboards and live tools, but avoid inventing a full reactive framework too early.

Proposed types:

```rust
pub struct SourceId(String);
pub struct SubscriptionId(String);
pub enum UpdateEvent { SourceChanged(SourceId), SourceError(SourceId), SourceEnded(SourceId) }
```

Requirements:

- Source IDs.
- Subscription handles.
- Update events through unified channel.
- Explicit unsubscribe/drop behavior.
- No global magic dependency graph.
- Components may opt into subscriptions but are not required to.

## Milestone 13 — Testing and quality

### Mock/test harness

Add a test harness that makes widget and app-shell behavior easy to test.

Requirements:

- Render widgets into ratatui `Buffer` snapshots.
- Simulate key/event streams.
- Simulate resize.
- Mock image surface recording `ensure_loaded`, `place`, `delete` calls.
- Mock scheduler or deterministic scheduler mode.
- Golden/snapshot helpers that are stable enough for CI.

### CI

GitHub Actions should run:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo doc --no-deps --all-features`
- optional examples build check

### Docs/examples

Minimum examples:

- explicit config startup;
- raw event loop;
- optional app shell/runtime;
- scheduler;
- picker with custom action map;
- scrollable list;
- table;
- tree;
- Kitty/WezTerm image placement;
- no-op image degraded mode;
- status bar/theme;
- component + dirty cache;
- focus/modal.

Each module doc should state:

- what `tui-kit` owns;
- what the app owns;
- extension points;
- configuration requirements;
- failure modes;
- examples of valid/invalid setup.

## Migration/rearchitecture sequence

Recommended order:

1. Add strict config/validation primitives and named presets.
2. Remove hardcoded picker `Tab` behavior via `PickerAction` key map.
3. Extract image placement policy from `ViewTransform::place` into configurable `PlacementEngine`.
4. Add `NoopImageSurface` and explicit image backend config for Kitty/disabled modes.
5. Categorize `AppEvent` while preserving one channel; optionally add generic `UserEvent`.
6. Add tick producers.
7. Add scheduler instrumentation and cancellation by ID/group/source/namespace.
8. Add scrollable list and table widgets.
9. Add mock terminal/image/scheduler test harness.
10. Add theme roles and widget style configs.
11. Add focus manager.
12. Add component trait and cached rendering.
13. Add tree/tabs/panes.
14. Reassess `AppShell` once enough examples repeat the same event-loop structure.
15. Add Sixel/iTerm2 after Kitty/WezTerm behavior is excellent and the surface abstraction has settled.

## Non-goals for now

- Human-readable minimalism as a priority. Clear machine-readable structure wins.
- Broad terminal image support before Kitty/WezTerm is solid.
- A full reactive framework.
- Hiding ratatui behind a complete replacement UI framework.
- Preserving current architecture when a better one unlocks the desired API.

## Specific known API corrections

- Remove app-specific picker group toggle from `Picker::handle_key`.
- Add configurable picker actions using `KeyMap<PickerAction>`.
- Keep `PickerItem<T>` generic and domain-free.
- Keep scheduler priority enum for now, but leave room for generic priorities.
- Add component trait.
- Add dirty tracking and cached rendering.
- Add tick/timer producers.
- Add focus traversal, modal capture, and restoration.
- Add generic data/update subscriptions later, cautiously.
- Add scrollable list, table, tree, tabs, panes.
- Add image backend selection, no-op/degraded mode, and eventually Sixel/iTerm2.
- Document image placement invariants.
- Keep one event channel, but categorize event payloads.
- Keep one scheduler, narrow in policy.
- Add cancellation granularity and queue instrumentation.
- Decide on optional app shell/runtime after repeated patterns are obvious.
- Keep terminal lifecycle ownership in toolkit.
- Add theme primitives with complete app override.
- Add mock terminal/test harness, CI, examples, and public docs.

## Success criteria

`tui-kit` is heading in the right direction when:

- an app can wire a complex UI without copying event/scheduler/focus/image lifecycle code;
- every important behavior is configured explicitly or installed via an explicit preset;
- misconfiguration fails early with useful structured errors;
- widgets expose mechanics without app semantics;
- image placement behavior is selectable per use case;
- Kitty/WezTerm image behavior is reliable;
- tests can verify text buffers and image placement side effects;
- AI agents can inspect the docs/config/API and make correct changes without guessing hidden defaults.
