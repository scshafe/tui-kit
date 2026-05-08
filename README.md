# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), provides the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. The API floor is what c4tui consumes; everything else is on probation. See `PLAN_REWRITE.md` for the design discipline.

## What's in the box

| Module | Provides | Consumer |
|---|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, tick, runtime/user events | c4tui |
| `component` | Optional `Component` / `BufferComponent` traits, `ComponentId`, dirty-state invalidation, `Cached<C>` buffer caching | _probationary — pending c4tui port_ |
| `focus` | `FocusManager` with stack-based modal/capturing scopes | _probationary — pending c4tui port_ |
| `input` | `Key` enum mapped from crossterm events | c4tui |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel | c4tui |
| `keymap` | `KeyMap` registry with `KeyTrigger → Command<C>` declarative bindings, last-binding-wins | c4tui |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ | c4tui |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle | c4tui |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement` — fit/zoom/pan math | c4tui |
| `bar` | `Segment` trait + `SegmentBar` registry — slot-aligned, priority-truncated text bars | c4tui |
| `scheduler` | Priority-queue task scheduler with custom-priority generic, scoped cancellation, machine-readable queue/timing stats | c4tui |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` | c4tui |
| `tick` | Named periodic tick producers with stop handles | _probationary — pending c4tui port_ |
| `widgets::picker` | Generic list-with-detail-and-thumbnails picker, fuzzy filter, scrollable, selection highlight | c4tui |
| `widgets::dialog` | Modal rendering plus policy-light dialog state with explicit confirm/cancel/focus actions | c4tui |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle | c4tui |
| `testkit` | Widget buffer rendering helpers, typed event scripts, mock image surface call recording | tests |

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Async runtimes (tokio/async-std) — uses sync threads + channels
- Full component tree runtime orchestration
- Theming, additional widgets (list/table/tree/tabs), runtime config bundles, subscription primitives — all deliberately removed pending consumer demand

## License

Dual-licensed under MIT or Apache-2.0.
