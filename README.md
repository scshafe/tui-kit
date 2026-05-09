# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), provides the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. The API floor is what c4tui consumes; the consumer-gate in CI fails the build if c4tui breaks. See `PLAN_REWRITE.md` for the design discipline.

## What's in the box

| Module | Provides | Consumer |
|---|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, user events | c4tui |
| `component` | `Component` / `BufferComponent` traits, `ComponentId`, dirty-state invalidation, `Cached<C>` buffer caching | c4tui (`ViewPicker`) |
| `focus` | `FocusManager` with stack-based modal/capturing scopes; `active_scope_id()` for distinguishing modal scopes | c4tui (modal stack — traversal API unused) |
| `input` | `Key` enum mapped from crossterm events | c4tui |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel | c4tui |
| `keymap` | `KeyMap<C>` registry generic over command type, `KeyTrigger → C` declarative bindings, last-binding-wins | c4tui (`KeyMap<PendingCommand>`) |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ | c4tui |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle | c4tui |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement` — fit/zoom/pan math | c4tui |
| `bar` | `StatusFragment`, `SegmentSlot`, `Segment<Ctx>` trait, `SegmentBar<Ctx>`, `layout_status_line` truncation | c4tui (data types + algorithm — `Segment<Ctx>` doesn't fit borrowed contexts; see open issue) |
| `scheduler` | Priority-queue task scheduler with custom-priority generic, scoped cancellation, machine-readable queue/timing stats | c4tui |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` | c4tui |
| `widgets::picker` | Generic list-with-detail-and-thumbnails picker, fuzzy filter, scrollable, selection highlight | _no consumer — c4tui has its own ViewPicker_ |
| `widgets::dialog` | `Dialog` widget for bordered modal text rendering; `DialogState` for confirm/cancel/focus actions | c4tui (uses `Dialog` only — `DialogState` unused) |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle | c4tui |
| `testkit` | Widget buffer rendering helpers, typed event scripts, mock image surface, `DeterministicScheduler` | tests/parity.rs |

## Open consumer-driven design questions

- `Segment<Ctx>` trait can't accept contexts with borrowed fields. c4tui's `StatusContext<'a>` falls back to a c4tui-internal trait. Resolving this likely requires a HRTB-friendly redesign or moving to owned contexts.
- `widgets::picker` has no consumer. c4tui's picker is c4tui-specific and lives in c4tui. Likely to be deleted unless a second consumer arrives.
- `focus::FocusTraversal` (Forward/Backward/Explicit) is unused. The traversal mechanics are not pressure-tested by any in-tree app.

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Async runtimes (tokio/async-std) — uses sync threads + channels
- Full component tree runtime orchestration
- Theming, additional widgets (list/table/tree/tabs), runtime config bundles, subscription primitives, periodic tick producers — all deliberately removed pending consumer demand

## License

Dual-licensed under MIT or Apache-2.0.
