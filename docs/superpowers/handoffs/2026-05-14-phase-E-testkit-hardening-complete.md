# Handoff — tui-kit Phase E (Testkit Hardening) complete

**Date:** 2026-05-14
**Repo:** tui-kit
**Scope:** This handoff covers tui-kit only. Phase E was independent of c4tui's
Phase F and required no cross-repo coordination.

## State

| Repo | Branch | HEAD | Tests | clippy | fmt | Origin |
|------|--------|------|-------|--------|-----|--------|
| `tui-kit` | `main` | `5fc66a4` | 162 lib + 11 parity passing | clean | clean | not pushed |

Verify locally:

```bash
cd /Users/coleshaffer/Projects/tui-kit
cargo test --quiet 2>&1 | grep "test result:"
cargo clippy --all-targets --quiet
cargo fmt --check
```

## What shipped in this session

Phase E (Testkit Hardening) — four commits.

| Commit | Subject |
|--------|---------|
| `dc6f8a8` | docs(plans): add Phase E bite-sized testkit-hardening plan |
| `1960bc0` | refactor(testkit): drop unreachable MockImageCall::Flush variant |
| `44625cd` | feat(testkit): RenderEffect assertion helpers and render_to_buffer |
| `5fc66a4` | docs(architecture): testkit vs c4tui FakeTerminalBackend in §10 |

### Commit details

- **`1960bc0` — drop unreachable `MockImageCall::Flush`.**
  `ImageSurface::flush` takes `&self`, and `MockImageSurface` derives
  `Clone + PartialEq + Eq`, so the mock cannot record a Flush call without
  breaking the derives. The variant was unreachable — no test could observe
  it. Removed with rustdoc explaining the modeling decision: flush is
  output-buffer flushing, not a lifecycle state change worth asserting on a
  mock that already records every lifecycle-affecting call.

- **`44625cd` — three new helpers, each with a same-commit consumer.**
  - `render_to_buffer<C: BufferComponent>(component, area) -> Result<Buffer>`
    collapses the recurring `Buffer::empty(area) + render_buffer(area, &mut
    buffer)?` boilerplate. Four sites in `src/elements/mod.rs` tests
    refactored to use it.
  - `find_place_with_placement_id(effects, placement_id) -> Option<&PlaceOptions>`
    replaces the `effects.iter().any(matches!(...))` pattern and surfaces the
    matched `PlaceOptions`. One site refactored.
  - `assert_teardown_covers(placed, teardown)` panics if any `(image_id,
    placement_id)` introduced by a `PlaceImage` in `placed` is not covered by
    a matching `DeleteImagePlacement` / `DeletePlacement` / blanket
    `DeleteAllPlacements` / `ForgetAllImages` in `teardown`. New test
    `stack_teardown_covers_every_rendered_placement` exercises it as the
    declarative form of the lifecycle invariant.

  Per Phase E's working rules: no helper shipped without a same-commit
  consumer.

- **`5fc66a4` — `architecture.md` §10 update.**
  Sharpens §10 to name the helpers as they exist post-commit-2 and draws
  the testkit vs `FakeTerminalBackend` boundary: tui-kit `testkit` asserts
  library-level invariants (buffer/effect boundary, render-effect shape,
  image lifecycle, scheduler determinism — all without entering a terminal);
  c4tui's `FakeTerminalBackend` asserts app-level wiring (the full
  `TerminalBackend` trait, enter/leave alt screen, render pipeline, modal
  scope routing).

## Plan-level Exit criteria (Phase E)

All three met:

1. ✅ Testkit exposes mock render/image surfaces and `RenderEffect` assertion
   helpers usable from `tests/` and from c4tui's tests. New helpers are
   `pub` in `tui_kit::testkit`; `MockImageSurface`/`MockImageCall` are honest
   about what they can record (the unreachable `Flush` variant is gone).
2. ✅ `architecture.md §10` says when to use tui-kit `testkit` versus c4tui's
   `FakeTerminalBackend`. New subsection added with the boundary rule.
3. ✅ Existing testkit users (`tests/parity.rs`, `src/widgets/grid.rs` tests,
   `src/widgets/image_box.rs` tests) continue to compile and pass.

## Parent plan exit criteria status (after Phase E)

The parent plan
(`docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md`)
has eight criteria. After Phase E:

1. ✅ `RenderEffect` is the only effect-enum name; data-only criteria hold. (Phase B)
2. ✅ `EffectElement` and `RenderEffect` in a dedicated module with rustdoc. (Phase B)
3. ✅ `architecture.md §8` and `specification.md §4.4/§5` use `RenderEffect`. (Phase B)
4. ❌ `src/elements/` ≤1,200 production lines or every retained widget has a named consumer — currently ~3,600 lines. Phase C not started.
5. ⚠️ c4tui placeholders cleared: NavPicker consolidation done (Tasks 1–4); Modal unification, image-viewport winner deletion, LinkDirectory, view-store split all pending.
6. ❌ Operator sign-off on local + SSH + container smoke tests — Phase D not started.
7. ✅ testkit verifies render output + render effects without a live terminal. (Phase E — closed in this session)
8. ❌ `architecture.md §2` widget table and prelude re-exports reflect surviving modules only — pending Phase C.

**4 of 8 met** (up from 3 before this session).

## What's next for tui-kit

Phases unblocked, in order of natural pickup:

- **Phase D — Image path + SSH/container reliability.** Now strongly
  positioned to use Phase E's new helpers: `MockImageSurface` already records
  lifecycle calls, `render_to_buffer` covers buffer assertions, and
  `assert_teardown_covers` plus `find_place_with_placement_id` are ready for
  image-lifecycle tests. Operator owns the live smoke tests; library-side
  scope is tests + env-var docs + operator scripts. Independent of c4tui's
  Phase F.

- **Phase C — Elements triage.** Still gated on c4tui Phase F Tasks 5–8
  (Modal unification, image-viewport winner deletion, LinkDirectory, view-store
  split) landing — those may pull in `Stack`, `Window`, `Modal`, `Overlay`,
  etc. before C can honestly delete.

## Cross-repo context

c4tui was unaffected by Phase E in this session. `tui_kit::testkit` is not
imported by c4tui (grep verified zero hits across `c4tui/src/`); the new
helpers are tui-kit-internal consumers today. c4tui adoption is opportunistic
and not required for Phase E exit.

## How to pick up tui-kit work from here

```bash
cd /Users/coleshaffer/Projects/tui-kit
git pull
# Read the operational plan:
cat docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md
# Read the Phase D pre-conditions section; the helpers from Phase E are ready.
```

The Phase E plan
(`docs/superpowers/plans/2026-05-14-phase-E-testkit-hardening.md`) remains
in place as historical record alongside the parent operational plan.
