# Direction — library author path and transport-safe render effects

**Status:** ACTIVE · 2026-05-13
**Supersedes:** the broader "distributed terminal graphics environment" framing
that treated a remote renderer protocol and `tui-kit-cli` as near-term roadmap
items.

## Thesis

tui-kit should be developed first as a high-quality Rust TUI library.

The distributed-rendering idea remains technically important, but it is not the
near-term product. The durable artifact is the library: typed input, pure buffer
rendering, image lifecycle management, render effects, layout math, and test
surfaces that other Rust terminal apps can depend on.

The right posture is library author, not protocol designer:

- A library can gain users one at a time.
- Library API mistakes can be corrected with SemVer and decisive deletion while
  the crate is early.
- A protocol needs multiple parties to agree up front, and mistakes are much
  more expensive.
- A transport-safe render contract is still valuable even if no transport is
  ever built.

## Direction Change

Earlier discussion correctly identified that tui-kit has the shape needed for a
future local renderer: buffer cells plus explicit host-side render operations.
The overreach was turning that immediately into a distributed shell/runtime
roadmap.

The narrowed direction:

- Make tui-kit excellent as a local/SSH-safe library.
- Make render effects data-only and transport-safe by construction.
- Ensure Kitty graphics survives common SSH and container paths.
- Let c4tui validate every architectural surface.
- Defer protocol, frame transport, persistent agents, and shell/runtime
  ambitions until concrete pain demands them.

## Three-Month Scope

### 1. Render-effect model

Rename `TerminalEffect` to `RenderEffect` when the code is ready. The name
should signal that these are render-host operations, not terminal-owned policy.

The effect enum must remain data-only:

- no callbacks;
- no closures;
- no arbitrary local commands;
- no hidden global terminal access;
- no app state references.

This keeps the contract transport-safe without committing to any wire format.

### 2. SSH and container reliability

The practical target is not "build a distributed renderer." The practical
target is: tui-kit apps do not break when run through SSH and containerized
development environments.

The load-bearing work is ordinary and important:

- `TERM` and `COLORTERM` propagation;
- terminfo availability;
- Kitty graphics passthrough behavior;
- `docker exec` and container PTY edge cases;
- clear degraded/no-image behavior when graphics are unavailable.

Live smoke testing across real SSH/container environments remains an operator
task, not an in-session agent task.

### 3. Testkit hardening

The render contract must be drivable without a live terminal:

- pure buffer rendering assertions;
- mock image/render surfaces;
- placement and teardown assertions;
- golden or snapshot fixtures where useful;
- deterministic scheduler/event tests.

This is how tui-kit preserves a future frame-transport path without prematurely
building transport.

### 4. c4tui as validating consumer

c4tui remains the proving ground. Every architectural addition should either be
used by c4tui, covered by tui-kit tests that encode a real invariant, or be
deleted.

Near-term c4tui-driven work still matters:

- unify picker code around `NavPicker`;
- converge on one image widget path;
- implement LinkDirectory;
- simplify modal and command/effect plumbing;
- split view catalog/render cache/viewport state when the consumer shape is
  clear.

## Non-Goals For Now

- No transport protocol.
- No frame format.
- No wire schema.
- No `tui-kit-agent`.
- No persistent host discovery, caching, or multiplexing daemon.
- No attempt to become a POSIX shell.
- No expansion of retained-widget/runtime machinery in `elements`.

These may be revisited later only if the library and c4tui work produce concrete
pressure for them.

## Elements

`elements` should be treated as a composition layer over `BufferComponent` plus
render effects.

The load-bearing pieces are:

- `BufferComponent` in `component.rs`;
- `Element` as a name for `BufferComponent<Event = KeyEvent>`;
- `RenderEffect` after the rename from `TerminalEffect`;
- `EffectElement`;
- `ElementExt` and simple decorator chains.

Cheap and useful sugar:

- `Text`;
- `Padding`;
- `ElementBorder`;
- `Padded`;
- `Bordered`;
- `ScrollY`;
- `Focusable`;
- `KeyMapped`.

Speculative or high-risk layer:

- `Panel`;
- `Stack`;
- `Window`;
- `Modal`;
- `Overlay`;
- `WindowChrome`;
- `KeyScope`;
- `ImageViewportElement`.

Do not preemptively delete these while c4tui is still in motion. Also do not
grow them. Keep them iff c4tui consumes them or tui-kit tests prove a durable
render/effect invariant. If the c4tui cleanup converges without them, delete
decisively.

The natural future seam for any transport is below `Window`: at
`BufferComponent + RenderEffect`. `Window` assumes a local render host through
lifecycle events, repaint policy, and render stats. That is acceptable as local
convenience; it should not define the transport boundary.

## WezTerm / Config Coordination

The seam with local terminal configuration should stay small: a versioned OSC
user-var schema for data such as project name, theme identity, capabilities, and
remote/session identifier.

tui-kit should not ship remote-controlled WezTerm logic. It may eventually emit
or document data payloads. The local WezTerm side remains responsible for
decoding and applying a conservative whitelist.

Remote sends data, never code.

## Operator Boundary

Agents can edit code, tests, and docs in-session.

The operator owns:

- live terminal smoke tests;
- real SSH tests;
- real container tests;
- WezTerm config integration;
- verifying Kitty graphics behavior in actual terminal stacks.

## Long-Term Pulls

Only after the library path is strong:

- optional frame transport;
- additional renderer backends;
- headless renderers for snapshots;
- versioned OSC payloads;
- binary-level capability negotiation.

These are pull-based possibilities, not committed near-term roadmap items.

