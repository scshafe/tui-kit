# tui-kit — Plan & Scope

This file is the static state for the tui-kit extraction effort and the long-term scope of the crate. It is the source of truth for "what's in tui-kit, what isn't, and where we're going." Update it as decisions are made.

## North star

A reusable Rust crate for building terminal UI applications with sophisticated rendering (Kitty graphics image embedding), async work scheduling, file watching, registry-driven UI patterns (status bars, key maps, pickers), and a clean event-driven application loop. Sits on `ratatui` for cell rendering and `crossterm` for cross-platform terminal I/O.

The crate is opinionated. It encodes patterns we've found valuable: unified event channels, mode-based input dispatch, transmit-once image lifecycle, priority-queue render scheduling, declarative key bindings.

## Concrete consumers

1. **c4tui** — interactive Structurizr C4 diagram viewer. Read-mostly, image-heavy (SVG → PNG → Kitty). Originating use case; first port target.
2. **(planned) dashboard** — real-time monitoring TUI. Write-heavy, chart-focused (text plots via ratatui or rendered via plotters → Kitty). Pressure-tests the dirty-tracking and tick-event paths.

## Phase plan

### Phase 0 — workspace setup ✓

- [x] Init repo at `/Users/coleshaffer/Projects/tui-kit`
- [x] Cargo.toml with deps (ratatui 0.29, crossterm 0.28, notify 6, fontdb 0.23, etc.)
- [x] README + this PLAN.md
- [ ] LICENSE files (MIT + Apache-2.0)

### Phase 1 — foundational extraction (no genericization)

Modules that have zero or minimal c4tui-specific coupling. Copy verbatim, adjust imports.

- [ ] `events.rs` — `AppEvent`, `AppEventSender/Receiver`. Currently has `RenderComplete { result: Result<RenderedView> }`. Generalize: `RenderComplete { id: u64, epoch: u64, result: Result<Box<dyn Any + Send>> }` so the scheduler isn't tied to SVG rendering.
- [ ] `input.rs` — `Key` enum + crossterm event translation
- [ ] `input_thread.rs` — detached input thread
- [ ] `keymap.rs` — generic over `Command<C>` (currently hard-coded to `PendingCommand`). Move to `KeyMap<C>` with `Command<C: Clone>`.
- [ ] `tty.rs` — `terminal_metrics`, `stdin/stdout_is_terminal`, `write_stdout_all`
- [ ] `image.rs` (was `kitty.rs`) — `KittyImageRegistry`, `PlaceOptions`, `ImageId`, placement-id helpers. Add `ImageSurface` trait so future Sixel/iTerm2 impls can drop in.

### Phase 2 — layout / bar / scheduler / watcher

Heavier extraction; some genericization needed.

- [ ] `layout.rs` — `PixelSize`, `CellSize`, `CellPixel`, `CanvasMetrics`, `PixelRect`, `CellRect`, `CellOffset`, `Placement`, `ViewTransform`, `ImagePoint`. Generic, no c4tui coupling. Verbatim.
- [ ] `bar.rs` (was `statusbar.rs`) — `Segment<Ctx>` trait generic over context. `SegmentBar<Ctx>` registry. `SegmentSlot` enum. Built-in segments (`AppNameSegment`, hint, etc.) move to a `bar::segments` module but are demoted from defaults — apps register their own.
- [ ] `scheduler.rs` (was `render_pool.rs`) — `Scheduler<T, R>` where `T` is request payload, `R` is result. Workers take a closure `Fn(T) -> Result<R>`. c4tui's `RenderScheduler` becomes a typedef `Scheduler<RasterRequest, RenderedView>`.
- [ ] `watcher.rs` — verbatim. Already generic.

### Phase 3 — widgets + terminal session

App-level UX pieces.

- [ ] `widgets/picker.rs` — generalize. Currently picker takes `&[ViewInfo]` directly. Refactor to `&[PickerItem<T>]` where `T` is opaque per-item data (returned on selection). Header/footer hints are configurable. Group labels come from a closure `Fn(&T) -> Option<&str>`. Thumbnail eligibility from a closure `Fn(&T) -> bool`.
- [ ] `widgets/dialog.rs` — modal box with title, message, footer hint. Pure ratatui.
- [ ] `widgets/picker_widget.rs` — the ratatui `Widget` impl for the picker (with thumbnail rect recording).
- [ ] `terminal.rs` — `Terminal` struct wrapping `ratatui::Terminal<CrosstermBackend<Stdout>>` + `KittyImageRegistry`. Owns raw-mode lifecycle. Exposes `draw(|frame| {...})`, `images()` accessor, `metrics()`, `translate_key(key)`.

### Phase 4 — c4tui port

c4tui depends on tui-kit (path dep during dev). Removes:

- `events.rs`, `input.rs`, `input_thread.rs`, `keymap.rs`, `tty.rs`, `kitty.rs`
- `layout.rs`, `statusbar.rs` (replaced by c4tui-specific segments using tui-kit's `Segment` trait)
- `picker.rs` (replaced by tui-kit's generic picker, fed c4tui's `PickerItem<ViewInfo>`)
- `render_pool.rs`, `watcher.rs`

Keeps:
- `app.rs` — c4tui-specific app loop, `AppMode`, command dispatching
- `cli.rs`, `config.rs`, `capabilities.rs`
- `event.rs` — c4tui's `Command` enum (Drill, Inspect, ShowLegend, etc.)
- `state.rs` — `AppState` (breadcrumbs, pinned element)
- `view.rs` — `ViewStore` (rendered cache)
- `workspace.rs` — Structurizr workspace parsing
- `render.rs` — SVG → PNG via resvg
- `ids.rs` — `ViewId`, `ElementId`
- `terminal.rs` — c4tui-specific render_view (uses tui-kit's `Terminal`)

## API conventions

- All public types `Debug + Clone` where reasonable.
- All registries follow the builder pattern: `Registry::builder().add(...).build()`.
- All events flow through `AppEventSender`. Producers don't share their own channels.
- All async work goes through `Scheduler`. Workers are pure functions.
- Lifetimes minimised — prefer `Arc<T>` over borrows for things that cross thread boundaries.
- Errors via `anyhow::Result` for app-level fallibility; rich errors via `thiserror` only when there's a clear consumer.
- No async runtime dependency. Threads + channels only.

## Decided trade-offs

| Decision | Choice | Rationale |
|---|---|---|
| Image protocol | Kitty only initially | User explicitly OK with that. Sixel/iTerm2 deferred. |
| Async runtime | None (sync threads) | Avoids tokio/async-std split, faster compile, simpler model. |
| Component model | Deferred (Phase 5+) | Dashboard hasn't materialized; `AppMode` handles c4tui's needs. Add `Component` trait when there's pressure. |
| Workspace structure | Separate repos | User has tui-kit and c4tui as separate repos. Path dep during dev, git dep before publishing. |
| Cross-platform | Yes by virtue of crossterm + ratatui | Image rendering tied to terminal capability, not OS. |

## Future scope (not blocking initial extraction)

- **Component trait** — `render(frame, area)`, `dirty()`, `mark_dirty()`, `children()`, `focus()`. With `Cached<C>` wrapper for retained-mode rendering. Pays off in dashboard.
- **Focus management** — `FocusStack` with Tab/Shift+Tab traversal. Modal nesting (modal A inside modal B).
- **Tick events** — configurable timer producer for periodic UI refresh. Required for dashboard's real-time updates.
- **Data subscription / pub-sub** — `DataSource<T>` + `Subscriber<T>`. Components register subscriptions; updates fire `AppEvent::DataUpdate(SourceId)`.
- **Sixel image surface** — for terminals without Kitty graphics. Falls back to ASCII-art if no image protocol available.
- **iTerm2 image protocol** — same, for Apple Terminal users.
- **Capability-driven surface selection** — runtime probe → picks Kitty/Sixel/iTerm2/Noop.
- **Theme system** — `Theme` struct loaded from TOML; widgets draw colors from it.
- **Keymap chord support** — `KeyTrigger::Sequence(Vec<KeyTrigger>)` for Vim-style `gg`, `dd`, etc.
- **Scrollable list widget** — separate from picker; for plain item lists with arrow-key scroll.
- **Tree widget** — for hierarchical data (dashboards with grouped metrics).
- **Test harness** — `MockTerminal` for snapshot testing widget output.
- **Performance instrumentation** — frame time, dirty-region stats, scheduler queue depth.
- **Plotter bridge** — `widgets::chart_image` that takes a `plotters::ChartContext` builder, renders to PNG via the scheduler, places via `ImageSurface`. For the dashboard.
- **Persistence helpers** — config dirs, cache dirs, with conventions matching `dirs` crate.

## Open questions

- **Scheduler genericization details.** Currently c4tui's `RenderScheduler` request type is `RenderRequest { view_id, priority, svg_path, budget }`. Generalizing to `Scheduler<T, R>` with `T = (id, priority, payload)` is straightforward. Do we want priority to be opaque to the scheduler, or do we standardize on a priority enum (Background/Hover/Active)? Standardize for now; apps can ignore unused priorities.
- **Picker item shape.** Current `PickerItem` has `view_id: ViewId, kind: ViewKind, name: String, key: String, description: Option<String>, element_names: Vec<String>`. Genericize to `PickerItem<T> { id: u64, primary: String, detail: Option<String>, group: Option<String>, payload: T }`. Search across primary/detail/group + a custom `searchable_extras` field for things like element names.
- **Bar context.** `StatusContext` in c4tui is a struct with view, transform, placement, canvas, etc. — Structurizr-specific. For tui-kit, the bar takes a generic `Ctx` (zero-sized for stateless segments, app-specific otherwise). Apps define their own context.
- **Where does `App` scaffolding live?** c4tui's `App::run` event loop is mostly generic except for command dispatching. Could move the loop scaffold into tui-kit (`AppShell` trait?) but the c4tui-specific commands are deeply integrated. Defer for now — let c4tui keep its `App::run`; revisit when dashboard arrives.

## Migration log

Updated as work progresses.

- 2026-05-06: PLAN.md created. Workspace setup begun.
