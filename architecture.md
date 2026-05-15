# tui-kit - Architecture

This document describes how tui-kit is organized internally. The behavioral
contract is described in [specification.md](./specification.md).

## 1. Overview

tui-kit is a library of composable layers rather than an application runtime.
Applications own their main loop, state, commands, and domain model. tui-kit
supplies reusable infrastructure around that loop:

```text
terminal/input backend
        |
        v
typed events -> app-owned command mapping -> app-owned state
        |                                      |
        |                                      v
        +----------------------------> tui-kit render/layout/effect helpers
                                                |
                                                v
                                renderer backend (local terminal today)
```

The central design rule is that reusable primitives stay domain-neutral. A
widget may know how to render a grid or place an image, but it does not know why
an application selected a row or what a selected image represents.

## 2. Module Map

| Module | Responsibility |
|---|---|
| `events` | Typed event envelope and sender aliases |
| `input` | Normalized key representation |
| `input_thread` | Blocking input reader that forwards input and resize events |
| `keymap` | Declarative key trigger to caller command mapping |
| `focus` | Stack-based modal/capturing focus scopes |
| `component` | Dirty-aware components and cached buffer rendering |
| `elements` | Composable buffer-rendered elements and explicit render-effect layers |
| `layout` | Cell/pixel metrics, placements, transforms, crops, viewport math |
| `image` | Kitty image registry and image-surface abstraction |
| `terminal` | Crossterm/ratatui terminal lifecycle plus image flushing |
| `widgets` | Dialog, grid, image box, and image viewport widgets |
| `bar` | Priority-aware status-line layout |
| `scheduler` | Priority background work queue and progress accounting |
| `watcher` | Debounced filesystem watcher |
| `tty` | Terminal size/capability probing helpers |
| `testkit` | Deterministic test utilities and mock surfaces |
| `prelude` | Common imports for app code |

## 3. Rendering Model

Buffer rendering is pure. Components and elements render into ratatui buffers
using the area they are given. They may return typed outcomes from input
handling, but rendering itself should not mutate terminal state outside the
buffer.

Terminal-facing rendering is explicitly planned and applied:

1. A widget computes text/border output for a ratatui buffer.
2. The widget or owner computes an image placement plan from source pixels,
   canvas metrics, zoom, and crop state.
3. Effect-capable elements expose image upload, placement, and teardown as
   explicit effect values.
4. A renderer backend applies those effects to an `ImageSurface` or another
   compatible surface.
5. Teardown is applied explicitly when an image leaves the screen or a mode
   closes.

This split keeps tests deterministic and prevents hidden terminal side effects
from leaking out of ordinary buffer rendering.

The same split is also the compatibility path for future remote rendering.
A client-side renderer can consume buffer cells and render effects over a
structured transport, while the application keeps the same event, layout, and
component boundaries. The current repository does not implement that transport;
the architecture should avoid closing the door on it.

## 4. Geometry Model

tui-kit keeps terminal cells and pixels distinct:

- `CellSize` and `CellArea` describe terminal grid dimensions.
- `PixelSize`, `PixelPoint`, and related types describe pixel dimensions.
- `CanvasMetrics` ties cells to pixels for one terminal snapshot.
- `Placement`, `ViewTransform`, and crop structures describe how source pixels
  map into a terminal region.

The image widgets and viewport helpers use these types to keep zoom, pan,
letterboxing, and source crop behavior testable without a terminal.

## 5. Component Model

`BufferComponent` is the primary reusable widget boundary:

- `render_buffer(area, buffer)` paints into a caller-provided buffer.
- `handle_event(event)` returns a `ComponentOutcome<Message>`.
- `dirty()` exposes repaint and image-placement invalidation state.

`Cached<C>` wraps a component and reuses its rendered buffer until dirty state
requires a repaint. Applications remain responsible for deciding when to draw,
where to place the component, and how to interpret component messages.

## 6. Focus and Modal Scope Model

`FocusManager` maintains a stack of named scopes. Each scope has a kind, such
as root, modal, or capturing. This lets an application route input based on the
active scope without baking app modes into the toolkit.

The focus layer tracks scope identity and nodes. It does not enforce traversal
policy beyond the current primitives; applications may layer their own selection
and keyboard behavior on top.

## 7. Terminal Boundary

The `terminal` module owns low-level terminal lifecycle responsibilities:

- enter and leave alternate screen;
- enable and disable raw mode;
- query terminal metrics;
- draw ratatui frames;
- manage Kitty image registry flushing;
- clean up terminal resources on drop.

Apps can bypass the high-level terminal wrapper and consume lower-level modules
directly if they need a custom backend.

The terminal wrapper is the concrete local backend, not the conceptual limit of
the rendering model. Future backends may apply the same buffers and render
effects from a client process, for example when an app runs on a remote host
and a local helper owns the real terminal.

### 7.1 Terminal-protocol requirements

For the existing Kitty image path to work end-to-end, the terminal stack must
satisfy a small, concrete set of requirements. These are protocol-level
requirements, not env-var requirements — tui-kit reads no env vars.

**Kitty graphics protocol.** The terminal must accept the Application Program
Command escapes that tui-kit writes for image upload, placement, and deletion.
The escape vocabulary is in `src/image.rs`:

- `transmit_png` (lines ~476-498) chunks PNG bytes into `\x1b_Ga=t,f=100,i=<id>,m=<more>;<base64>\x1b\\` APC payloads.
- `kitty_place_escape` (lines ~505-518) emits `\x1b_Ga=p,i=<id>,p=<pid>,q=2,x=,y=,w=,h=,c=,r=;\x1b\\`.
- `kitty_delete_placement_escape` (lines ~501-503) emits `\x1b_Ga=d,d=i,i=<id>,p=<pid>,q=2;\x1b\\`.
- `KittyImageRegistry::forget_all` (lines ~451-458) emits `\x1b_Ga=d,d=I,i=<id>,q=2;\x1b\\` per loaded image.
- `KittyImageRegistry::shutdown` (lines ~391-393) emits the global `\x1b_Ga=d,d=A,q=2;\x1b\\` cleanup.

Terminals that drop these escapes silently, or that translate them to literal
output, will exhibit no images but no error either — the failure mode is
visual, not crashy.

**Alt-screen + raw-mode lifecycle.** `Terminal::enter_with_config`
(`src/terminal.rs:115`) enables raw mode, enters the alternate screen, hides
the cursor, and enables mouse capture before any image escape is written.
`Drop` restores all of these in reverse order and calls
`ImageSurfaceRegistry::shutdown` to free image data. Tests must not enter a
real alternate screen; the testkit's `MockImageSurface` and
`render_to_buffer` exercise the data flow without touching the terminal.

**Cursor positioning for `place_at`.** Kitty places images at the current
cursor position. `position_cursor` (`src/image.rs:466-474`) issues `\x1b[r;cH`
before each `place` call routed through `ImageSurfaceRegistry::place_at`.
Terminals or PTY layers that filter cursor-position escapes will mis-place
images.

**TTY detection.** The only runtime check tui-kit does is `IsTerminal` on
stdin/stdout (`src/tty.rs:14-20`). There is no terminfo probing, no `TERM`
sniffing, no `COLORTERM` check. Backend selection is configuration-driven
via `TerminalConfig::image_backend` (`src/terminal.rs:45-48`) and its preset
constructors.

**Window size.** `terminal_metrics` (`src/tty.rs:28-42`) calls `TIOCGWINSZ`
to get both cell and pixel dimensions. If the PTY layer does not propagate
pixel dimensions, the metrics fall back to `CellPixel::FALLBACK` and image
sizing degrades gracefully.

Operator-side verification of these requirements against real terminals is
documented in `docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`.

## 8. Render Effects and Image Lifecycle

The image layer is centered on stable image IDs and placement IDs. Callers can:

- transmit image bytes once;
- place the same image in different terminal regions;
- update source crops for pan/zoom;
- delete placements or images explicitly.

This model favors responsive interactions where a large image is cached in the
terminal and only placement/source-rectangle data changes.

`elements` exists to preserve the composition side of this model: containers
that transform child areas must also transform effect origins and group
teardown. It should stay narrow. It should not grow into a retained component
tree runtime, app state container, or product-specific UI framework.

## 9. Scheduler and Watcher

The scheduler is a generic priority queue. Work runs outside the foreground UI
path and reports completion through the shared event channel. Progress data is
structured so applications can display queue depth, active work, and completed
items without parsing logs.

The watcher wraps notify with debounce and emits workspace-change events. It is
not tied to any particular file format.

## 10. Testing Architecture

The testkit (`src/testkit.rs`) mirrors the public boundaries:

- render `ratatui::Widget` and `BufferComponent` implementations into buffers
  via `render_widget`, `render_stateful_widget`, and `render_to_buffer`;
- script typed `AppEvent` streams via `EventScript`;
- assert image-lifecycle calls via `MockImageSurface` + `MockImageCall`;
- assert render-effect shape via `find_place_with_placement_id` and
  `assert_teardown_covers` (the latter encodes the "everything placed is
  torn down" invariant declaratively);
- run deterministic scheduler flows via `DeterministicScheduler`.

The data-flow lifecycle scenarios for the image path are locked by tests in
`src/image.rs` (alongside the `NoopImageSurface` lifecycle test): load-then-
place, place-then-resize-area, teardown-then-place, repeated place with
stable placement id, `forget_all` cycle requiring reload, and a full
lifecycle through `ImageBackendPreference::Disabled` that exercises the
degraded path without panic.

Tests should verify terminal-facing behavior at the boundary rather than by
entering a real alternate screen.

### `tui-kit::testkit` vs `c4tui::backend::FakeTerminalBackend`

Two test seams exist; they have different jobs and should not be confused.

- **`tui-kit::testkit` asserts library-level invariants.** It tests the
  buffer/effect boundary, render-effect sequences, image-lifecycle calls, pure
  buffer rendering, and scheduler determinism — everything reachable through
  the library's public traits (`BufferComponent`, `EffectElement`,
  `ImageSurface`, `Scheduler`). It does not enter or fake a terminal: a
  `MockImageSurface` records calls, a `DeterministicScheduler` runs work on
  demand, and `render_to_buffer` produces an owned buffer without any
  alternate-screen lifecycle.

- **`c4tui::backend::FakeTerminalBackend` asserts app-level wiring.** It
  tests the full `TerminalBackend` trait: enter/leave alternate screen, the
  render pipeline plumbing, modal scope routing, and anything that requires
  the terminal lifecycle. It is c4tui's responsibility because that lifecycle
  is what an app — not a library — owns.

Rule of thumb: assert library invariants in tui-kit `tests/` or `src/.../tests`
modules with `testkit`. Assert app wiring in c4tui tests with
`FakeTerminalBackend`. If a tui-kit test is reaching for a terminal lifecycle,
the test belongs on the c4tui side or the production code under test is
crossing a boundary it shouldn't.

## 11. Extension Rules

New public surfaces should meet at least one of these tests:

- they remove repeated code from more than one caller;
- they encode a terminal or layout invariant that is easy to get wrong;
- they preserve the buffer/effect boundary needed by local, test, or future
  remote renderer backends;
- they expose a smaller, more testable boundary for an existing toolkit
  responsibility.

Domain-specific behavior belongs in the consuming application, not in tui-kit.
