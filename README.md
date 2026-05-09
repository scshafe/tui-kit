# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), provides the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. The API floor is what c4tui consumes; the consumer-gate in CI fails the build if c4tui breaks. See `PLAN_REWRITE.md` for the design discipline.

## What's in the box

| Module | Provides | Consumer |
|---|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, user events | c4tui |
| `component` | `Component` / `BufferComponent` traits, `ComponentId`, dirty-state invalidation, `Cached<C>` buffer caching | c4tui (`ViewPicker`) |
| `focus` | `FocusManager` with stack-based modal/capturing scopes; `active_scope_id()` for distinguishing modal scopes | c4tui (modal stack) |
| `input` | `Key` enum mapped from crossterm events | c4tui |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel | c4tui |
| `keymap` | `KeyMap<C>` registry generic over command type, `KeyTrigger → C` declarative bindings, last-binding-wins | c4tui (`KeyMap<PendingCommand>`) |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ | c4tui |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle | c4tui |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement` — fit/zoom/pan math | c4tui |
| `bar` | `StatusFragment`, `SegmentSlot`, `layout_status_line` priority truncation | c4tui |
| `scheduler` | Priority-queue task scheduler with custom-priority generic, scoped cancellation, machine-readable queue/timing stats | c4tui |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` | c4tui |
| `widgets::dialog` | `Dialog` widget for bordered modal text rendering | c4tui |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle | c4tui |
| `testkit` | Widget buffer rendering helpers, typed event scripts, mock image surface, `DeterministicScheduler` | tests/parity.rs |

## Removed pending consumer demand

These public surfaces were pruned after the c4tui migration showed no real consumer. Reintroduce any of them only with a named consumer in the same change set.

- `widgets::picker`: c4tui uses its own app-specific `ViewPicker`; no real consumer validated the generic picker API.
- `widgets::dialog` modal state/config/actions: c4tui only consumes the presentational `Dialog` widget.
- `bar::Segment<Ctx>` / `SegmentBar<Ctx>` registry: c4tui consumes `StatusFragment`, `SegmentSlot`, and `layout_status_line`; app-owned segment traits fit borrowed status contexts better.
- Generic focus traversal: c4tui consumes modal scope/capture only.

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Async runtimes (tokio/async-std) — uses sync threads + channels
- Full component tree runtime orchestration
- Generic focus traversal, theming, additional widgets (list/table/tree/tabs), runtime config bundles, subscription primitives, periodic tick producers — all deliberately removed pending consumer demand

## License

Dual-licensed under MIT or Apache-2.0.
