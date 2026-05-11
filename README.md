# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), provides the layers most apps rebuild from scratch.

## Status

Early. Extracted from [c4tui](https://github.com/scshafe/c4tui) as the reusable substrate. The API floor is what c4tui consumes; the consumer-gate in CI fails the build if c4tui breaks. See `PLAN_REWRITE.md` for the design discipline.

## What's in the box

| Module | Provides | Consumer |
|---|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, user events | c4tui |
| `component` | `Component` / `BufferComponent` traits, `ComponentId`, dirty-state invalidation, `Cached<C>` buffer caching | c4tui (`ViewPicker`) |
| `elements` | First-class buffer-rendered `Element`s, decorators, `Text`, `Panel`, `Window`, `Stack`, overlays, and typed terminal effects | c4tui migration target |
| `focus` | `FocusManager` with stack-based modal/capturing scopes; `active_scope_id()` for distinguishing modal scopes | c4tui (modal stack) |
| `input` | `Key` enum mapped from crossterm events | c4tui |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel | c4tui |
| `keymap` | `KeyMap<C>` registry generic over command type, `KeyTrigger → C` declarative bindings, last-binding-wins | c4tui (`KeyMap<PendingCommand>`) |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ | c4tui |
| `image` | `KittyImageRegistry` + `ImageSurface` trait — transmit-once-place-many image lifecycle | c4tui |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement`, `TailViewport` — fit/zoom/pan math plus consumer-backed tail-scroll viewport math | c4tui |
| `bar` | `StatusFragment`, `SegmentSlot`, `layout_status_line` priority truncation | c4tui |
| `scheduler` | Priority-queue task scheduler with custom-priority generic, scoped cancellation, machine-readable queue/timing stats | c4tui |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` | c4tui |
| `widgets::dialog` | `Dialog` widget for bordered modal text rendering | c4tui |
| `widgets::grid` | Selectable/grid collection renderer with local cell canvases, active/selected styling, keyboard navigation, and scroll indicators | c4tui picker migration target |
| `widgets::image_box` | `ImageBox`, `ImageBoxState`, and `ImageBoxPlan` — common image viewport: source dimensions, zoom, crop, optional border/title | image-box tests / visual test |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle | c4tui |
| `testkit` | Widget buffer rendering helpers, typed event scripts, mock image surface, `DeterministicScheduler` | tests/parity.rs |

## Element composition

Elements render into ratatui buffers. `Panel` is presentational chrome,
`Window` is the lifecycle/key/effect boundary, `Stack` lays out children, and
`Grid` is the selectable collection primitive.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use tui_kit::prelude::*;

fn render_root(area: Rect) -> anyhow::Result<Buffer> {
    let body = Text::with_id("body", "Build log ready").wrap(true);
    let panel = Panel::new("body-panel", body).title("Status").padding(1);
    let mut window = Window::new("main-window", panel)
        .with_title("Workspace")
        .with_padding(1);
    window.activate();

    let footer = Text::with_id("footer", "q quit  / filter").with_padding((1, 0));
    let mut root = Stack::vertical("root")
        .with_child(window, StackConstraint::Fill(1))
        .with_child(footer, StackConstraint::Length(1));

    let mut buffer = Buffer::empty(area);
    root.render(area, &mut buffer)?;
    Ok(buffer)
}

fn render_picker(area: Rect, buffer: &mut Buffer, items: &[&str], active: Option<usize>) {
    Grid::new()
        .with_columns(1)
        .with_active_index(active)
        .render(area, buffer, items, |cell, canvas| {
            let marker = if cell.active { "> " } else { "  " };
            canvas.set_string(0, 0, format!("{marker}{}", cell.item), Style::default());
        });
}
```

Effectful children stay explicit. Use the effect-aware child/layer APIs when a
container needs to forward terminal side effects:

```rust,ignore
let mut media = Stack::vertical("media").with_effect_child(image, StackConstraint::Fill(1));

for effect in media.terminal_effects(area)? {
    effect.apply_to_registry(terminal.images())?;
}

for effect in media.teardown_effects()? {
    effect.apply_to_registry(terminal.images())?;
}
```

`scroll_y` is buffer-only for now. Scrolled image/effect children need explicit
clipping and source-cropping semantics before they can be forwarded safely.

## ImageBox

`ImageBox` is the streamlined image viewport. It keeps source image dimensions, applies zoom to derive theoretical dimensions, then crops the theoretical image to the available box pixels. If the theoretical image is smaller than the box on either axis, it is centered on that axis.

It does not replace the lower-level primitives. Consumers that need direct control can still use the `layout` and `image` modules directly.

```rust
use tui_kit::prelude::*;

let image = ImageBox::new(image_id, MAIN_PLACEMENT_ID, PixelSize::new(width, height))
    .border(true)
    .title("Diagram");
let mut state = ImageBoxState::default();

let plan = image.plan(area, terminal.metrics(), &state)?;
terminal.draw(|frame| {
    plan.render(frame.buffer_mut());
})?;
plan.place(terminal.images())?;
terminal.images().flush()?;
```

Manual visual test:

```bash
cd visual-tests
cargo run --offline
```

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
