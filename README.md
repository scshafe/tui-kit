# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), adds the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. API will change.

## What's in the box

| Module | Provides |
|---|---|
| `events` | `AppEvent` enum + unified channel: keys, render-completion, file-change, resize, ticks |
| `input` | `Key` enum mapped from crossterm events |
| `input_thread` | Detached input thread that pushes `AppEvent::Key`/`Resize` into the unified channel |
| `keymap` | `KeyMap` registry with `KeyTrigger → Command<C>` declarative bindings, last-binding-wins |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement` — fit/zoom/pan math |
| `bar` | `Segment` trait + `SegmentBar` registry — slot-aligned, priority-truncated text bars |
| `scheduler` | Priority-queue task scheduler with epoch-based cancellation |
| `watcher` | notify-based file watcher with debounce, emits `AppEvent::WorkspaceChanged` |
| `widgets::picker` | Generic list-with-detail-and-thumbnails picker, fuzzy filter, scrollable, selection highlight |
| `widgets::dialog` | Modal with title, message, footer hint |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle |

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Async runtimes (tokio/async-std) — uses sync threads + channels
- Component tree with retained dirty tracking — Phase 3 work, deferred until needed
- Theming system, plugin loading, etc.

## License

Dual-licensed under MIT or Apache-2.0.
