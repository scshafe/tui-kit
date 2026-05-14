# Fresh Plan — tui-kit library-author path

**Status:** ACTIVE · 2026-05-13
**Direction doc:** [`2026-05-13-library-author-direction.md`](./2026-05-13-library-author-direction.md)
**Supersedes:** the previous roadmap as the forward planning entrypoint.

## Goal

Build tui-kit into an excellent Rust TUI library that works locally, works
through SSH/container environments where the terminal supports it, and preserves
a transport-safe render/effect contract without committing to a transport
protocol.

The near-term validating consumer remains c4tui.

## Principles

- Library first. Protocol later, if pulled by real use.
- c4tui validates architecture. Speculative APIs are deleted.
- Render effects are data, not behavior.
- Buffer rendering stays pure.
- Image lifecycle stays explicit.
- SSH/container work means "do not break," not "build a distributed shell."
- Live terminal and real SSH/container testing are operator-only.

## Phase A — Baseline and Direction Cleanup

**Purpose:** Make sure the docs and plans no longer point future workers toward
an oversized distributed-runtime commitment.

- [x] Mark older roadmap material as superseded by this plan.
- [x] Keep the direction doc and core spec/architecture aligned.
- [x] Ensure Phase 3 planning says `elements` is preserved only as a
      render/effect substrate, not expanded into a framework.
- [x] Keep `tui-kit-cli`, transport protocols, and agents out of active scope.
- [x] Run doc consistency checks for stale "delete elements" instructions in
      active plans.

**Exit:** future workers can start from this plan without reading the whole
conversation history.

## Phase B — RenderEffect Contract

**Purpose:** Promote the effect contract as tui-kit's load-bearing idea.

- [ ] Rename `TerminalEffect` to `RenderEffect` when the diff is controlled.
- [ ] Keep an adapter method for applying effects to `ImageSurfaceRegistry`.
- [ ] Ensure `RenderEffect` is data-only and serializable in principle.
- [ ] Keep effect application separate from effect description.
- [ ] Add or preserve tests that assert image upload, placement, deletion, and
      teardown behavior through mock surfaces.
- [ ] Document `RenderEffect + EffectElement` as the core innovation.

**Non-goals:** no frame transport, no wire schema, no client helper.

**Exit:** tui-kit can describe render-host operations without saying "terminal"
where it means "renderer."

## Phase C — Elements Triage

**Purpose:** Keep the useful composition layer and constrain the retained-widget
surface.

- [ ] Keep `Element` as the ergonomic alias/name for
      `BufferComponent<Event = KeyEvent>`.
- [ ] Keep `ElementExt` and cheap decorators if tests remain simple and useful.
- [ ] Keep `Text`, `Padding`, `ElementBorder`, `Padded`, `Bordered`,
      `ScrollY`, `Focusable`, and `KeyMapped` unless a real consumer/test says
      otherwise.
- [ ] Do not add new retained-widget/runtime concepts.
- [ ] Demote `Panel`, `Stack`, `Window`, `Modal`, `Overlay`, `WindowChrome`,
      `KeyScope`, and `ImageViewportElement` to "keep iff consumed or testing a
      hard render/effect invariant."
- [ ] If c4tui does not consume retained widgets after cleanup, delete them
      decisively.
- [ ] If readability requires it, split `RenderEffect + EffectElement` into a
      smaller module; do not split files for its own sake.

**Exit:** `elements` is either small and justified, or large pieces have been
deleted because no consumer needed them.

## Phase D — Image Path and SSH/Container Reliability

**Purpose:** Make the current terminal image path robust in the environments the
operator actually uses.

- [ ] Keep one production image viewport path; delete the loser after c4tui
      validates the winner.
- [ ] Verify graceful degraded/no-image behavior.
- [ ] Document required terminal environment variables and terminfo assumptions.
- [ ] Add tests for image lifecycle and placement math that do not require a
      real terminal.
- [ ] Add operator smoke-test scripts or checklists for:
      - local Kitty/WezTerm;
      - SSH;
      - SSH into container;
      - `docker exec` or equivalent PTY path.
- [ ] Record known terminal/container passthrough edge cases.

**Exit:** the library has a clear, tested image lifecycle and a practical manual
test path for real SSH/container graphics.

## Phase E — Testkit Hardening

**Purpose:** Make the future transport-safe contract testable without transport.

- [ ] Strengthen mock render/image surfaces.
- [ ] Add helper assertions for render effects.
- [ ] Keep pure buffer rendering tests easy to write.
- [ ] Add golden/snapshot fixtures only where they reduce ambiguity.
- [ ] Document when to use tui-kit testkit vs c4tui fake backend.

**Exit:** a consumer can verify render output and render effects without a live
terminal emulator.

## Phase F — c4tui Validation Work

**Purpose:** Continue the consumer cleanup that proves which tui-kit surfaces are
actually worth keeping.

- [ ] Finish `NavPicker` consolidation.
- [ ] Unify modal handling around the smallest useful abstraction.
- [ ] Keep the selected image viewport path wired through c4tui.
- [ ] Implement LinkDirectory as the keyboard-first navigation surface.
- [ ] Collapse command/effect duplication where Phase 3 left it.
- [ ] Split view catalog/render cache/viewport state once picker and
      LinkDirectory dependencies are narrow.
- [ ] Delete tui-kit APIs c4tui no longer consumes unless tests encode a
      library-level invariant.

**Exit:** c4tui is clean enough that tui-kit can distinguish durable library
surface from consumer-specific convenience.

## Deferred Pulls

These are explicitly not active phases:

- `tui-kit-agent`;
- full frame transport;
- remote renderer protocol;
- `dgksh` as shell/session supervisor;
- persistent discovery/caching/multiplexing;
- non-terminal renderers.

Reopen only with a concrete use case that cannot be solved by the library path
plus ordinary SSH/container reliability.

## Working Rules

- Do code, tests, and docs in-session.
- Do not claim live SSH/container verification was performed unless the operator
  performed it.
- Keep commits scoped.
- Prefer deletion over compatibility shims while the crate is early.
- Every new public surface needs a named consumer or a test proving a terminal,
  layout, render-effect, or lifecycle invariant.
