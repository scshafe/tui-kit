# tui-kit - Specification

This document describes what tui-kit provides. The implementation structure is
described in [architecture.md](./architecture.md).

## 1. Purpose

tui-kit is a reusable Rust crate for building terminal applications on top of
ratatui and crossterm. It provides the lower-level services that terminal apps
commonly rebuild: input normalization, keymaps, focus scopes, dirty-aware
components, cell/pixel layout math, inline image lifecycle management,
background scheduling, file watching, status-line layout, and test utilities.
The current runtime target is a local terminal session, but the public
boundaries should remain structured enough to support a future local-renderer
client for remote applications.

tui-kit is intentionally domain-neutral. It does not know what kind of data an
application displays, how that data is loaded, or what commands mean.

## 2. Goals

- Keep terminal protocol details behind small, explicit APIs.
- Support deterministic app tests without entering raw mode or emitting image
  escape sequences.
- Treat cell geometry and pixel geometry as first-class values.
- Make image transmission and placement explicit so callers control lifecycle.
- Preserve a structured render/effect boundary so terminal-facing behavior can
  be adapted to local terminals, test surfaces, and future remote renderers.
- Provide reusable widgets and primitives without imposing an application
  runtime, state model, or domain object model.
- Prefer synchronous, dependency-light building blocks that can be composed by
  command-line applications.

## 3. Non-goals

- A full application framework or retained component tree runtime.
- A domain-specific viewer, editor, browser, dashboard, or document model.
- Async runtime integration as a requirement.
- Automatic persistence, settings discovery, or app-specific configuration
  schemas.
- Browser, GUI, or web rendering.
- SSH clients, remote-render protocols, or persistent agents in this
  specification version.
- Image protocols other than the Kitty graphics protocol unless a real consumer
  justifies the added surface area.

## 4. Public Surface

### 4.1 Events

The `events` module defines a typed `AppEvent<UserEvent>` envelope for input,
terminal, scheduler, watcher, and caller-defined events. Apps own their event
loop and decide how events map to commands.

### 4.2 Input and Keymaps

The `input` module normalizes crossterm events into `KeyEvent`, `MouseEvent`,
and `InputEvent`. The `input_thread` module can spawn a blocking input reader
that forwards keyboard, mouse, and resize events into an app event channel.

The `keymap` module maps typed triggers to caller-owned command values. Last
binding wins so applications can layer user overrides on top of defaults.

### 4.3 Focus

The `focus` module provides stack-based focus scopes. Scopes can represent root
content, modal overlays, or capturing modes. tui-kit tracks scope identity and
focus nodes; applications decide what those nodes mean.

### 4.4 Components and Elements

The `component` module defines dirty-aware component traits and the `Cached<C>`
wrapper for buffer caching. Components render into ratatui buffers and report
typed outcomes to their owner.

The `elements` module provides composable buffer-rendered elements and explicit
render effects. Its durable purpose is to keep area-transforming composition
and terminal-facing effects together without hiding those effects in pure
buffer rendering. It is not a retained application runtime.

### 4.5 Layout

The `layout` module defines typed cell and pixel geometry, canvas metrics,
placements, view transforms, source crops, and tail-scroll viewport math.
Callers use these primitives to translate between terminal cells, terminal
pixels, source images, and application coordinate systems.

### 4.6 Images and Terminal

The `image` module defines the Kitty image registry and the `ImageSurface`
boundary. The registry supports transmit-once, place-many image workflows and
explicit teardown.

The `terminal` module wraps ratatui/crossterm terminal lifecycle setup,
alternate-screen/raw-mode cleanup, image flushing, and terminal metrics.

Future renderer backends may consume the same buffers and render effects through
a structured transport instead of writing terminal escape sequences directly.
That protocol is not specified here, but current APIs should avoid assuming
that every renderer is the process-local `stdout`.

### 4.7 Widgets

The `widgets` module contains reusable widgets:

- `dialog` for simple bordered text overlays.
- `grid` for selectable collections with active/selected styling, keyboard
  movement, and scroll indicators.
- `image_viewport` for image placement, zoom, and crop planning.

Widgets should expose behavior through typed state and outcomes rather than
owning application commands.

### 4.8 Scheduling and Watching

The `scheduler` module provides a priority queue for background work with
machine-readable progress and timing data. The `watcher` module provides a
debounced filesystem watcher that emits typed workspace-change events.

### 4.9 Status Lines

The `bar` module lays out left and right status fragments by priority, width,
and truncation policy. It does not own app-specific segment registries.

### 4.10 Testkit

The `testkit` module provides buffer rendering helpers, event scripts, mock
image surfaces, and deterministic scheduler tools for tests.

## 5. Behavioral Contracts

- Pure buffer rendering must not emit terminal escape sequences.
- Terminal effects, image placement, and teardown must be explicit values or
  explicit method calls.
- Effect-carrying composition must keep render intent serializable in principle:
  no ambient access to terminal globals, application state, or arbitrary local
  commands.
- Dirty state should distinguish repaint needs from image-placement changes
  where widgets can do so cheaply.
- APIs crossing application boundaries should use typed IDs or typed geometry
  instead of raw tuples where practical.
- Tests must be able to exercise components without a live terminal emulator.

## 6. Compatibility

tui-kit targets terminal applications using ratatui 0.29 and crossterm 0.28.
The crate is early and may still revise APIs, but changes should preserve the
core split between domain-neutral primitives and caller-owned application
behavior. Local terminal rendering is the implemented compatibility target
today; remote-render clients remain future work.

## 7. Out of Scope for This Specification

This document does not define how any particular application loads data, names
commands, stores navigation state, or chooses visual design. Those decisions
belong to consuming applications.
