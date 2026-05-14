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

The testkit mirrors the public boundaries:

- render widgets into buffers and assert cells;
- script typed events;
- use mock image surfaces to assert image placement and teardown;
- assert render-effect forwarding through area-transforming containers;
- run deterministic scheduler flows.

Tests should verify terminal-facing behavior at the boundary rather than by
entering a real alternate screen.

## 11. Extension Rules

New public surfaces should meet at least one of these tests:

- they remove repeated code from more than one caller;
- they encode a terminal or layout invariant that is easy to get wrong;
- they preserve the buffer/effect boundary needed by local, test, or future
  remote renderer backends;
- they expose a smaller, more testable boundary for an existing toolkit
  responsibility.

Domain-specific behavior belongs in the consuming application, not in tui-kit.
