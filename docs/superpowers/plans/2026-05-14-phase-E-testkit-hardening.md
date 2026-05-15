# Phase E — Testkit Hardening (Bite-Sized Plan)

**Status:** ACTIVE · 2026-05-14
**Parent:** [`2026-05-14-revised-library-author-implementation-plan.md`](./2026-05-14-revised-library-author-implementation-plan.md)
**Predecessor in graph:** Phase B (shipped).
**Concurrent with:** c4tui's ongoing Phase F (NavPicker / Modal consolidation).
**Independence:** Does not block, and is not blocked by, Phase D or Phase F.

## Goal (lifted from parent plan §"Phase E")

Make the future transport-safe contract testable without transport, with helper
assertions for render-effect shape. Concretely: close the `MockImageSurface`
gap, add `RenderEffect` sequence assertion helpers, give `BufferComponent`
rendering a one-call ergonomic, and document when to use tui-kit `testkit`
versus c4tui's `FakeTerminalBackend`.

## Why this is the next on-path work

Phase D will need exactly the helpers Phase E builds (image-lifecycle
assertions, `RenderEffect` sequence matchers) to write tests cleanly. Building
E first means D inherits the tools instead of retrofitting them, and the work
is purely library-internal — no operator coordination, no env-var documentation
that risks drift. **Confidence: HIGH.**

## Pre-conditions and verified state

- `src/testkit.rs` (617 lines) exposes `MockImageSurface`, `MockImageCall`,
  `DeterministicScheduler`, `render_widget`, `render_stateful_widget`,
  `test_area`, `test_cell_pixels`, `EventScript`. Verified by reading the file.
- `MockImageSurface` implements the full `ImageSurface` trait, but
  `MockImageSurface::flush(&self)` cannot push to `self.calls: Vec<_>` — the
  trait method is `&self` and the mock derives `Clone + PartialEq + Eq`, ruling
  out `RefCell`/`Mutex` interior mutability without breaking the derives.
  Result: `MockImageCall::Flush` is **unreachable code** — no test can ever
  observe it. **Confidence: HIGH** (grep verified, no consumer found).
- `BufferComponent::render_buffer(&mut self, area, buffer) -> Result<()>` is
  called from 20+ test sites with the boilerplate `let mut buffer =
  Buffer::empty(area); component.render_buffer(area, &mut buffer)?;`. A thin
  `render_to_buffer` helper collapses this. **Confidence: HIGH.**
- The patterns Phase E's helpers replace are concrete:
  - `effects.iter().any(|e| matches!(e, RenderEffect::PlaceImage { options, .. } if options.placement_id == 9))`
    at `src/elements/mod.rs:3415` and similar.
  - `assert_eq!(teardown, vec![RenderEffect::DeleteImagePlacement { image_id, placement_id }])`
    at `src/elements/mod.rs:3420-3427` and `:3443-3449`.
- `c4tui` carries `FakeTerminalBackend` at `c4tui/src/backend.rs:55-87`. It
  does **not** consume `RenderEffect`, `EffectElement`, or `tui_kit::testkit`
  today (grep verified). Phase E's helpers will be tui-kit-internal consumers
  in this phase; c4tui adoption is opportunistic, not required for exit.

## Non-goals (inherited)

- No transport. No serialization helpers beyond `Debug + PartialEq` round-trip
  checks (the data-only contract already proves serializability in principle).
- No async / runtime-specific helpers.
- No event-loop test harness in tui-kit.
- No expansion of `testkit` API beyond helpers with named consumers in this
  phase's commits.

## Commit plan (3 commits, behavior-preserving)

### Commit 1 — `testkit`: remove unreachable `MockImageCall::Flush` variant

**File:** `src/testkit.rs`

- Remove `MockImageCall::Flush` from the enum.
- Update the rustdoc on `MockImageCall` to state the modeling decision:
  `ImageSurface::flush(&self)` is intentionally not recorded because (a) it
  takes `&self` per the trait, ruling out push-to-Vec from the derived-`Eq`
  mock, and (b) `flush` represents output-buffer flushing, not a lifecycle
  state change worth asserting on a mock that already records every
  lifecycle-affecting call.
- Verify no `MockImageCall::Flush` references exist anywhere
  (`grep MockImageCall::Flush src/`).

**Verification:** `cargo test --quiet` green; `cargo clippy --all-targets`
clean.

**Rationale:** Honest API. Closes a public-surface gap that would otherwise
mislead a future reader into thinking flush is observable on the mock.

### Commit 2 — `testkit`: `RenderEffect` assertion helpers + `render_to_buffer`

**Files:** `src/testkit.rs`, `src/elements/mod.rs` (test refactors only).

Add three helpers, each with a proof-of-consumer refactor:

1. **`find_place_for(effects, image_id, placement_id) -> Option<&PlaceOptions>`**
   - Returns the `PlaceOptions` payload of the first `RenderEffect::PlaceImage`
     in `effects` matching both ids, or `None`.
   - Consumer: refactor `window_groups_effect_teardown_without_duplicate_child_teardown`
     at `src/elements/mod.rs:3404-3430` to use it.

2. **`assert_teardown_covers(placed: &[RenderEffect], teardown: &[RenderEffect])`**
   - For every `(image_id, placement_id)` pair appearing in a
     `PlaceImage` variant in `placed`, panic with a descriptive message if
     `teardown` does not contain *either* a matching `DeleteImagePlacement`,
     *or* a `DeletePlacement` for that `placement_id`, *or* a
     `DeleteAllPlacements`, *or* a `ForgetAllImages`.
   - Consumer: add a new test in `src/elements/mod.rs` that asserts the
     `Window` teardown invariant declaratively using this helper. Use the
     existing `ImageViewportElement` fixture (lines 3404-3430 area).
   - Rationale: this encodes the lifecycle invariant "everything placed gets
     torn down" once, where today it's open-coded per test.

3. **`render_to_buffer<C: BufferComponent>(component: &mut C, area: Rect) -> Result<Buffer>`**
   - Collapses `let mut buffer = Buffer::empty(area); component.render_buffer(area, &mut buffer)?; buffer` into one call.
   - Consumer: refactor 3–5 sites in `src/elements/mod.rs` tests (e.g.
     `text_renders_*`, `padded_renders_*`, `bordered_renders_*` style tests
     near lines 2883–3120). Pick sites that are pure render-and-assert; skip
     sites that need the intermediate `&mut Buffer` for layered renders.

**Module shape:** put the two `RenderEffect` helpers in `src/testkit.rs` under
a `pub mod render_effects` submodule (or top-level functions if cleaner — pick
at write time, prefer the option with the shortest call-site). `render_to_buffer`
lives next to `render_widget` at module top-level.

**Verification:** `cargo test --quiet` green (refactored tests must pass
unchanged in behavior); `cargo clippy --all-targets` clean.

**No helpers without consumers:** if any of the three lacks a real
in-this-commit consumer, omit it. The plan above commits to consumers; do not
add unused surface.

### Commit 3 — `architecture.md §10`: testkit vs c4tui fake backend guidance

**File:** `architecture.md`

Append a subsection (or sharpen the existing §10) that says:

- `tui-kit::testkit` is for **library-level invariants**: render-effect shape,
  image-lifecycle calls (via `MockImageSurface`), pure buffer rendering (via
  `render_widget` / `render_to_buffer`), scheduler determinism (via
  `DeterministicScheduler`).
- `c4tui::backend::FakeTerminalBackend` is for **app-level wiring**: terminal
  lifecycle (enter/leave alt screen), render pipeline plumbing, modal scope
  routing, and anything that requires the full `TerminalBackend` trait.
- One-line rule: assert library invariants in tui-kit `tests/` or
  `src/.../tests` modules with `testkit`; assert app wiring in c4tui tests
  with `FakeTerminalBackend`.

Reference the new helpers by name and module path so a reader can grep to
their definitions.

**Verification:** `cargo doc --no-deps` for any rustdoc cross-refs the doc
introduces (none expected, but check). Spot-check that the new section's
references to file paths and trait names still match the code.

## Plan-level Exit criteria (Phase E, from parent plan)

After all three commits land:

- ☐ Testkit exposes mock render/image surfaces and `RenderEffect` assertion
  helpers usable from `tests/` and from c4tui's tests (the helpers are
  `pub`, the `MockImageSurface` and `MockImageCall` API is honest about what
  it can record).
- ☐ `architecture.md §10` says when to use tui-kit `testkit` versus c4tui's
  `FakeTerminalBackend`.
- ☐ Existing testkit users (`tests/parity.rs`, `widgets/grid.rs` tests,
  `widgets/image_box.rs` tests) continue to compile and pass.

## Working rules (inherited)

- Each commit is small, scoped, and behavior-preserving. Refactor commits do
  not introduce new behavior; assertion-helper commits expose new API only.
- No graceful-migration shims, no deprecated re-exports.
- No new public surface without a same-commit consumer (tui-kit `tests/`,
  `src/.../tests`, or c4tui — c4tui not required).
- Commit messages follow tui-kit's lowercase-imperative style.

## Risks and mitigations

- **Risk: `assert_teardown_covers` is too lax — it accepts
  `DeleteAllPlacements` as covering everything, which could hide bugs.**
  *Mitigation:* documented in rustdoc on the helper that `DeleteAllPlacements`
  / `ForgetAllImages` are accepted because they represent the explicit
  "tear down everything" idiom; tests that want stricter assertions can use
  `find_place_for` directly or pattern-match on `teardown` themselves.
- **Risk: refactoring test sites in `src/elements/mod.rs` masks a regression
  by changing what the test asserts.** *Mitigation:* refactor commit must
  preserve the assertion semantics exactly. If a test's assertion shape
  doesn't fit the new helper cleanly, leave it alone; do not stretch the
  helper or weaken the assertion.
- **Risk: `render_to_buffer` returns `Result<Buffer>` but most callers use
  `unwrap()` or `?`. The `?` form requires the test to return `Result`.**
  *Mitigation:* tests that already return `Result<()>` (most do) use `?`
  naturally; tests that don't return `Result` keep the explicit two-line form.
  Pick refactor sites accordingly.
- **Risk: `render_effects` submodule name collides with the
  `EffectElement::render_effects()` method.** *Mitigation:* if the collision
  is awkward at call sites, name the submodule `effect_assertions` or move
  the helpers to top-level `testkit` functions like
  `find_place_image_for(...)`.

## What this plan explicitly does **not** decide

- Whether `MockImageSurface` should grow interior mutability to track `flush`
  (no — the modeling decision is "flush is not a state change worth
  recording"; this can be revisited if a transport surfaces a need).
- Whether to add golden/snapshot fixtures (no — the parent plan biases
  toward structural assertions; goldens can land later if a test would
  otherwise be ambiguous).
- Whether c4tui adopts the new helpers (out of scope; c4tui adoption is
  opportunistic and Phase F's call).
