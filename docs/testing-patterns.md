# Cross-Repo Testing Patterns

This document describes the two test seams shared by tui-kit and c4tui. Keep
library invariants in tui-kit and app lifecycle assertions in c4tui.

## tui-kit `testkit`

Use `tui_kit::testkit` when the behavior can be expressed through tui-kit's
public library boundaries:

- render a `ratatui::Widget` or `BufferComponent` into an owned buffer;
- script typed `AppEvent` values;
- assert image lifecycle calls through `MockImageSurface`;
- assert renderer backend consumption through `MockRendererBackend`;
- assert `RenderEffect` sequences;
- drive scheduler work deterministically.

Example:

```rust
use ratatui::layout::Rect;
use tui_kit::component::{BufferComponent, ComponentOutcome};
use tui_kit::input::KeyEvent;
use tui_kit::testkit::render_to_buffer;

fn renders_component_text<C>(component: &mut C) -> anyhow::Result<String>
where
    C: BufferComponent<Event = KeyEvent>,
{
    let buffer = render_to_buffer(component, Rect::new(0, 0, 20, 1))?;
    Ok(buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>())
}

fn assert_ignored<M>(outcome: ComponentOutcome<M>) {
    assert!(matches!(outcome, ComponentOutcome::Ignored));
}
```

Reach for this seam when a test is about buffer output, image-effect shape,
renderer-frame consumption, typed geometry, scheduler ordering, or a toolkit
trait contract. Do not enter a real terminal or assert c4tui app state here.

## c4tui `FakeTerminalBackend`

Use `c4tui::backend::fake::FakeTerminalBackend` when the behavior needs the
c4tui application lifecycle:

- render a `RenderFrame` through `TerminalBackend`;
- assert modal render/close sequencing;
- assert image viewport teardown requests;
- assert help, error, message, and image-cache effects;
- run app-level command/effect integration tests without raw mode.

Example:

```rust
use c4tui::app::App;
use c4tui::backend::fake::{FakeTerminalBackend, FakeTerminalCall};
use tui_kit::input::KeyEvent;

fn press(app: &mut App, terminal: &mut FakeTerminalBackend, key: KeyEvent) {
    app.handle_key(terminal, key).expect("key handling should not fail");
}

fn assert_modal_rendered(terminal: &FakeTerminalBackend) {
    assert!(terminal.calls.contains(&FakeTerminalCall::RenderModal));
}
```

Reach for this seam when a test is about c4tui orchestration: focus scopes,
modal ownership, navigation effects, reload behavior, or the terminal backend
trait. Do not use it to test generic buffer math that belongs in tui-kit.

## Boundary Rule

If a test can be written against `BufferComponent`, `EffectElement`,
`RendererBackend`, `ImageSurface`, or `Scheduler`, keep it in tui-kit. If it needs
`App`, `AppState`, `TerminalBackend`, `ViewCatalog`, `RenderCache`, or
`ViewportState`, keep it in c4tui.
