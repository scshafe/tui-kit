# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), adds the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. API will change.

## What's in the box

| Module | Provides |
|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, tick, runtime/user events |
| `component` | Optional component traits, stable IDs, focus node handles, explicit dirty-state invalidation, and safe buffer caching for opt-in components |
| `input` | `Key` enum mapped from crossterm events |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel |
| `keymap` | `KeyMap` registry with `KeyTrigger → Command<C>` declarative bindings, last-binding-wins |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement` — fit/zoom/pan math |
| `bar` | `Segment` trait + `SegmentBar` registry — slot-aligned, priority-truncated text bars |
| `scheduler` | Priority-queue task scheduler with scoped cancellation and machine-readable queue stats |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` |
| `tick` | Named periodic tick producers with explicit validation and stop handles |
| `widgets::list` | Policy-light scrollable list mechanics with optional selection, exposed viewport math, explicit key actions |
| `widgets::table` | Policy-light table mechanics with stable row/column IDs, sizing policies, row selection, and vertical/horizontal viewport math |
| `widgets::tabs` | Policy-light tab state, close/reorder request hooks, pane split sizing policies, focus metadata, and inspectable pane layout results |
| `widgets::tree` | Policy-light hierarchical state with expand/collapse, lazy-child hooks, optional tri-state checkboxes, stable IDs, and flattened viewport math |
| `widgets::picker` | Generic list-with-detail-and-thumbnails picker, fuzzy filter, scrollable, selection highlight |
| `widgets::dialog` | Modal rendering plus policy-light dialog state with explicit confirm/cancel/focus actions |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle |

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Async runtimes (tokio/async-std) — uses sync threads + channels
- Full component tree runtime orchestration
- Theming system, plugin loading, etc.

## License

Dual-licensed under MIT or Apache-2.0.
