# tui-kit

Opinionated middleware for building terminal UI applications. Sits on top of [`ratatui`](https://ratatui.rs/) and [`crossterm`](https://github.com/crossterm-rs/crossterm), provides the layers most apps rebuild from scratch.

## Status

Early. The crate is a domain-neutral substrate for terminal applications that need typed input, focus scopes, buffer-rendered components, explicit render effects, image placement, layout math, scheduling, and test utilities without adopting a full application framework. Local terminal sessions are the implemented target today; the buffer/effect split is intended to keep future remote renderers possible.

## Documents

- [specification.md](./specification.md) - public behavior, goals, and non-goals.
- [architecture.md](./architecture.md) - module structure, data flow, and extension rules.

## What's in the box

| Module | Provides |
|---|---|
| `events` | Typed `AppEvent<UserEvent>` categories + unified channel: input, terminal, scheduler, watcher, user events |
| `component` | `BufferComponent` trait, `ComponentId`, dirty-state invalidation, `Cached<C>` buffer caching |
| `elements` | First-class buffer-rendered `Element`s, area-transforming containers, overlays, and explicit render/terminal effects |
| `focus` | `FocusManager` with stack-based modal/capturing scopes; `active_scope_id()` for distinguishing modal scopes |
| `input` | `KeyEvent`, `MouseEvent`, and `InputEvent` mapped from crossterm events |
| `input_thread` | Detached input thread that pushes `InputEvent::Key` and `TerminalEvent::Resize` into the unified channel |
| `keymap` | `KeyMap<C>` registry generic over command type, `KeyTrigger -> C` declarative bindings, last-binding-wins |
| `tty` | `terminal_metrics()` reading both cell and pixel dimensions via TIOCGWINSZ |
| `image` | `KittyImageRegistry` + `ImageSurface` trait - transmit-once-place-many image lifecycle |
| `layout` | `PixelSize`, `CellSize`, `CanvasMetrics`, `ViewTransform`, `Placement`, `TailViewport` - fit/zoom/pan math plus tail-scroll viewport math |
| `bar` | `StatusFragment`, `SegmentSlot`, `layout_status_line` priority truncation |
| `scheduler` | Priority-queue task scheduler with custom-priority generic, scoped cancellation, machine-readable queue/timing stats |
| `watcher` | notify-based file watcher with debounce, emits `WatcherEvent::WorkspaceChanged` |
| `widgets::dialog` | `Dialog` widget for bordered modal text rendering |
| `widgets::grid` | Selectable/grid collection renderer with local cell canvases, active/selected styling, keyboard navigation, and scroll indicators |
| `widgets::image_box` | `ImageBox`, `ImageBoxState`, and `ImageBoxPlan` - common image viewport: source dimensions, zoom, crop, optional border/title |
| `terminal` | `Terminal` wrapping `ratatui::Terminal<CrosstermBackend>` + image registry + raw-mode lifecycle |
| `testkit` | Widget buffer rendering helpers, typed event scripts, mock image surface, `DeterministicScheduler` |

## Element composition

Elements render into ratatui buffers and keep terminal-facing effects explicit.
`Panel` is presentational chrome, `Window` currently groups lifecycle and
effect teardown, `Stack` lays out children, and `Grid` is the selectable
collection primitive.

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
container needs to forward image placement, teardown, or other renderer-facing
effects:

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

This effect path is intentionally separate from direct terminal writes. Today
effects apply to the local `ImageSurfaceRegistry`; later they can become the
wire intent consumed by a local renderer for a remote app.

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

These public surfaces were pruned after they showed no durable consumer demand. Reintroduce any of them only with a named consumer in the same change set.

- `widgets::picker`: no real consumer validated the generic picker API.
- `widgets::dialog` modal state/config/actions: the presentational `Dialog` widget is the reusable part.
- `bar::Segment<Ctx>` / `SegmentBar<Ctx>` registry: app-owned segment traits fit borrowed status contexts better.
- Generic focus traversal: modal and capturing scopes are the validated reusable
  pieces today.

## Out of scope (today)

- Image surfaces other than Kitty graphics (Sixel, iTerm2)
- Remote renderer protocol, SSH launcher, and client-side helper binaries
- Async runtimes (tokio/async-std) - uses sync threads + channels
- Full component tree runtime orchestration
- Generic focus traversal, theming, additional widgets (list/table/tree/tabs), runtime config bundles, subscription primitives, periodic tick producers - all deliberately removed pending consumer demand

## License

Dual-licensed under MIT or Apache-2.0.
