# Post Phase 17 Architecture Follow-Up Handoff

**Status:** READY FOR NEXT PLANNING SESSION, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Current heads when written:**

- `tui-kit`: `cb8b028 Document Phase 17 hygiene plan`
- `c4tui`: `6e1107d Clean up API hygiene and app tests`

This handoff converts the post-Phase 17 adversarial architecture review into a
high-level action roadmap. It is not an implementation plan. The next session
should expand these action items into detailed phase plans before code changes
begin.

`/Users/coleshaffer/Projects/tui-kit/visual-tests/` remains operator-owned and
must not be touched unless explicitly requested.

## Baseline

Phases 11-17 are implemented and verified:

- Phase 11: c4tui modal factory and effect runner.
- Phase 12: tui-kit elements module split.
- Phase 13: tui-kit remote transport/session boundary.
- Phase 14: c4tui workspace module split.
- Phase 15: image geometry boundary cleanup.
- Phase 16: terminal image output and capability probe IO boundaries.
- Phase 17: public API and test fixture hygiene.

Latest verification from Phase 17:

- `tui-kit`: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo test`, `cargo doc --no-deps`, and
  `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`.
- `c4tui`: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
  and `cargo test`.
- Public surface audit passed with 152 items checked.
- `c4tui` was clean; `tui-kit` was clean except the pre-existing
  untracked `visual-tests/` directory.

## Read First

Required context for the next session:

- `docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`
- `docs/superpowers/plans/2026-06-13-phase-10-code-quality-assessment.md`
- `docs/superpowers/plans/2026-06-14-phase-11-c4tui-app-orchestration-modal-factory.md`
- `docs/superpowers/plans/2026-06-14-phase-12-tui-kit-elements-module-split.md`
- `docs/superpowers/plans/2026-06-14-phase-13-remote-transport-session.md`
- `docs/superpowers/plans/2026-06-14-phase-14-c4tui-workspace-module-split.md`
- `docs/superpowers/plans/2026-06-14-phase-15-image-geometry-boundary-cleanup.md`
- `docs/superpowers/plans/2026-06-14-phase-16-terminal-io-capability-probe-boundaries.md`
- `docs/superpowers/plans/2026-06-14-phase-17-public-api-test-fixture-hygiene.md`
- this handoff.

High-value code files to inspect while planning:

- `c4tui/src/app.rs`
- `c4tui/src/effect_runner.rs`
- `c4tui/src/backend.rs`
- `c4tui/src/terminal.rs`
- `c4tui/src/state.rs`
- `c4tui/src/viewport_state.rs`
- `tui-kit/src/layout.rs`
- `tui-kit/src/widgets/image_viewport.rs`
- `tui-kit/src/remote.rs`
- `tui-kit/src/terminal.rs`
- `tui-kit/src/image.rs`

## Operating Constraints

- No backwards-compatibility shims, deprecated aliases, or migration layers.
- Preserve behavior unless an action item explicitly proposes changing it.
- Keep public re-exports only for final APIs.
- Keep `visual-tests/` untouched unless explicitly requested.
- Plan first; do not implement Phases 18+ until the detailed plans exist and
  the user asks for implementation.
- Each eventual implementation phase should be independently verifiable and
  committed before the next phase begins.

## High-Level Action Roadmap

### Phase 18 - c4tui Runtime Transaction Boundary

**Problem:** `App` is much smaller after Phase 11 and Phase 17, but it still
owns event routing, modal-key transitions, config cycling, root rendering,
reload reset, and render scheduling. Reload/store reset logic exists in both
`App` and `EffectRunner`.

**Direction:** Introduce a single runtime/transaction boundary. Command and
modal handling should produce explicit render/effect intents, and one executor
should decide whether to render root, render modal, close modal, show dialog,
clear image cache, or request renders.

**Planning questions:**

- Should the boundary be named `AppRuntime`, `RuntimeContext`, or
  `RenderTransaction`?
- Which code should remain in `App` after extraction?
- How should config cycling flow through the same effect path as state
  commands?
- Can reload logic be represented once and reused for manual and watcher
  reloads?

**Expected verification:** c4tui fmt, clippy, tests. tui-kit verification only
if public or dependency behavior changes.

### Phase 19 - c4tui Terminal Rendering and Backend Split

**Problem:** `TerminalBackend` is broad, and `TerminalSession` owns root layout,
status/footer text, image placement, modal rendering, thumbnail cleanup,
dialogs, help text, and image cache operations.

**Direction:** Split rendering responsibilities into focused internal units:
root canvas renderer, modal renderer, status/footer renderer, image placement
painter, and dialog/help presenter. Narrow `TerminalBackend` so tests assert
render commands without inheriting every terminal concern.

**Planning questions:**

- Should the fake backend track high-level render transactions instead of every
  current terminal method?
- Which pieces belong in tui-kit versus c4tui?
- Can modal thumbnail placement cleanup become a typed modal render result
  rather than terminal-side catalog enumeration?

**Expected verification:** c4tui fmt, clippy, tests; tui-kit fmt, clippy, tests
if any toolkit boundary moves.

### Phase 20 - Image Geometry Canonicalization

**Problem:** Image geometry ownership is improved but still split across
`tui-kit::layout`, `tui-kit::widgets::image_viewport`, and
`c4tui::viewport_state`. c4tui bridges `ViewTransform`, `PlacementPolicy`, and
`ImageViewportWidget` state, which increases the risk of drift.

**Direction:** Make one model canonical. Prefer keeping pure policy and
placement math in `tui-kit::layout`, while c4tui stores only product state and
delegates all pan, zoom, placement, and hit-test math through the same engine.

**Planning questions:**

- Is `ImageViewportWidget` still the right state carrier for c4tui, or should
  c4tui store `ViewTransform` plus `PlacementEngine`?
- Which convenience methods on `ViewTransform` still encode too much default
  policy?
- How can hit testing and render placement share the same computed placement
  without redoing cache/viewport work?

**Expected verification:** tui-kit fmt, clippy, tests, cargo doc, public-surface
audit; c4tui fmt, clippy, tests.

### Phase 21 - c4tui Internal Visibility Hygiene

**Problem:** c4tui is a binary crate but still has many top-level `pub` items.
Phase 17 removed stale dead surfaces; it did not fully tighten crate-local
visibility.

**Direction:** Convert default internal surfaces to `pub(crate)` or private
where possible. Keep `pub` only when the module boundary genuinely requires it.
Consider adding an `unreachable_pub` check once the churn is controlled.

**Planning questions:**

- Which modules need cross-module exports and which only expose to tests?
- Should test modules stay as child modules to access private surfaces?
- Can any remaining type aliases in modal/status/navigation become private
  implementation detail?

**Expected verification:** c4tui fmt, clippy, tests. Run tui-kit verification
only if dependency/public usage changes.

### Phase 22 - Remote Protocol Typed Framing

**Problem:** Phase 13 added a transport/session boundary, but
`tui-kit/src/remote.rs` still documents the protocol as a prototype and uses a
line-oriented `TKF1` frame format.

**Direction:** Add typed protocol messages, explicit version negotiation, and
stricter stream framing before adding host profiles, reconnect behavior, or
more remote effects.

**Planning questions:**

- Should the next wire format use serde with length-prefix framing, or keep
  line framing with typed JSON messages?
- How should compatibility be handled given the project rule of no migration
  layers?
- What test transport fixtures prove startup, first-frame, passthrough, EOF,
  malformed frame, and child-exit behavior?

**Expected verification:** tui-kit fmt, clippy, tests, cargo doc, public-surface
audit; c4tui fmt, clippy, tests only if dependency behavior changes.

### Phase 23 - Test Fixture Architecture Cleanup

**Problem:** App tests are now separated, but fixture-heavy tests remain spread
across c4tui modules. tui-kit `testkit` is powerful and public, so new fixture
promotion must be careful.

**Direction:** Keep test helpers local by default. Promote only genuinely
reused helpers to c4tui test support or `tui_kit::testkit`, and avoid turning
one-off test setup into public API.

**Planning questions:**

- Which c4tui fixtures are duplicated across app/state/modal/render-cache tests?
- Should c4tui add private `test_support` modules behind `#[cfg(test)]`?
- Do any tui-kit `testkit` helpers need pruning or stronger retention docs?

**Expected verification:** both repos fmt, clippy, and tests; tui-kit doc/audit
if `testkit` public surface changes.

## Recommended Sequencing

1. Phase 18 first. It addresses the highest orchestration risk and will make
   later terminal/backend work smaller.
2. Phase 19 second. It depends on knowing what render transactions should look
   like.
3. Phase 20 third. Geometry changes are high-value but riskier; do them after
   c4tui render orchestration has fewer branches.
4. Phase 21 fourth. Visibility tightening is safest after the main c4tui
   boundaries settle.
5. Phase 22 fifth. Remote work is important but less urgent than local runtime
   quality; avoid expanding the protocol while local rendering is still moving.
6. Phase 23 last, unless fixture duplication blocks earlier phases.

## Next Session Objective

The next session should produce detailed implementation plans for these action
items. It should not start implementation unless the user explicitly asks.

Suggested plan documents:

- `docs/superpowers/plans/2026-06-14-phase-18-c4tui-runtime-transaction-boundary.md`
- `docs/superpowers/plans/2026-06-14-phase-19-c4tui-terminal-rendering-backend-split.md`
- `docs/superpowers/plans/2026-06-14-phase-20-image-geometry-canonicalization.md`
- `docs/superpowers/plans/2026-06-14-phase-21-c4tui-internal-visibility-hygiene.md`
- `docs/superpowers/plans/2026-06-14-phase-22-remote-protocol-typed-framing.md`
- `docs/superpowers/plans/2026-06-14-phase-23-test-fixture-architecture-cleanup.md`

Each detailed plan should include:

- concrete scope and non-scope;
- exact files to inspect and edit;
- behavior-preservation notes;
- risks and mitigation;
- TDD or characterization tests where useful;
- verification commands for each repo;
- commit scope.

## Copyable Next-Session Prompt

```text
Pick up from the post-Phase 17 architecture follow-up handoff.

Repos:
- /Users/coleshaffer/Projects/tui-kit
- /Users/coleshaffer/Projects/c4tui

Read first:
- /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-14-post-phase-17-architecture-followup-handoff.md
- /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-13-phase-10-code-quality-assessment.md
- /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-14-phase-17-public-api-test-fixture-hygiene.md

Current completed commits when the handoff was written:
- tui-kit: cb8b028 Document Phase 17 hygiene plan
- c4tui: 6e1107d Clean up API hygiene and app tests

Task:
Create detailed implementation plans, but do not implement yet, for the
post-Phase 17 action roadmap:
1. Phase 18 - c4tui Runtime Transaction Boundary
2. Phase 19 - c4tui Terminal Rendering and Backend Split
3. Phase 20 - Image Geometry Canonicalization
4. Phase 21 - c4tui Internal Visibility Hygiene
5. Phase 22 - Remote Protocol Typed Framing
6. Phase 23 - Test Fixture Architecture Cleanup

Write each detailed plan under:
/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/

Constraints:
- No backwards-compatibility shims, deprecated aliases, or migration layers.
- Preserve current behavior unless a detailed plan explicitly calls out a
  behavior change for approval.
- Keep edits scoped to planning; do not start implementation.
- Do not touch /Users/coleshaffer/Projects/tui-kit/visual-tests/ unless I
  explicitly ask.
- Each future implementation phase must be independently verifiable and
  committed before the next phase starts.

After writing the plans, report the recommended implementation order and any
open questions that need operator input before Phase 18 begins.
```

## Verification for This Handoff

This handoff is documentation only. Required checks:

- `git diff --check`
- `git status --short -- visual-tests`
- commit this handoff document in `tui-kit`.
