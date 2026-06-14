# Handoff — tui-kit Phase D (Image Reliability) library-side complete

**Date:** 2026-05-15
**Repo:** tui-kit
**Scope:** This handoff covers tui-kit only. The operator-run smoke checklist
(Phase D exit criterion #3) is gated on a separate operator-signed handoff
that will land once the operator runs `docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`.

## State

| Repo | Branch | HEAD | Tests | clippy | fmt | Origin |
|------|--------|------|-------|--------|-----|--------|
| `tui-kit` | `main` | `eb98784` | 168 lib + 11 parity passing | clean | clean | not pushed |

Verify locally:

```bash
cd /Users/coleshaffer/Projects/tui-kit
cargo test --quiet 2>&1 | grep "test result:"
cargo clippy --all-targets --quiet
cargo fmt --check
```

## What shipped in this session

Phase E completion (recorded in the prior handoff) plus Phase D library-side
in four commits.

| Commit | Subject |
|--------|---------|
| `d1dd1f6` | docs(plans): add Phase D bite-sized image-reliability plan |
| `8320085` | test(image): lock data-flow lifecycle scenarios via MockImageSurface |
| `7345ccc` | docs(architecture): terminal-protocol requirements for Kitty path |
| `eb98784` | docs(handoffs): operator smoke-test checklist for image path |

### Commit details

- **`8320085` — lifecycle tests.** Six new tests in `src/image.rs` lock the
  call-sequence invariants the parent plan names: load-then-place,
  place-then-resize (stable placement_id, different cell dims),
  teardown-then-place, repeated place with identical options, `forget_all`
  cycle requiring reload, and `ImageBackendPreference::Disabled` registry
  full lifecycle (closes the gap where selection was tested but the
  registry-level lifecycle wasn't).

- **`7345ccc` — `architecture.md §7.1`.** New subsection answers the four
  questions a reader needs about the Kitty image path: what the terminal
  stack must accept (the APC escape vocabulary, with file:line pointers to
  `transmit_png`, `kitty_place_escape`, `kitty_delete_placement_escape`,
  `forget_all`, `shutdown`); what tui-kit assumes about alt-screen + raw-mode
  lifecycle (owned by `Terminal::enter_with_config`; reversed on `Drop`);
  what env vars tui-kit consults (*none* — only `IsTerminal` runtime check);
  and where the escape vocabulary lives in code. §10 also updated to point at
  the new lifecycle tests.

- **`eb98784` — operator smoke-test checklist.** Four sections (local
  Kitty/WezTerm, SSH, SSH→docker exec, local docker exec) with setup, run,
  expected, failure modes, and results placeholders. Drives c4tui as the
  test vehicle (tui-kit has no image example today). Pre-populated
  "Known passthrough edge cases" from code reading. Sign-off block at the
  bottom — until the operator runs and signs, status is "scripted,
  awaiting operator."

## Plan-level Exit criteria (Phase D)

- ✅ `cargo test` covers the image lifecycle without a live terminal. Test
  inventory documented in `architecture.md §10` (commits 1 + 2).
- ✅ Required env vars / terminfo entries are documented — the env-var
  inventory is honestly "none read"; the *real* terminal-protocol
  requirements are spelled out in §7.1 (commit 2).
- ⏳ Operator smoke checklist exists. Checklist ships in commit 3; operator
  sign-off is gated on the operator running it and recording results.
- ✅ Graceful degraded path is asserted by a test, not just claimed by docs
  — `disabled_registry_full_lifecycle_does_not_panic` exercises the
  `ImageBackendPreference::Disabled` path through every `ImageSurface`
  method on the production registry (commit 1).

**3 of 4 closed**; the operator-run sign-off is the 4th.

## Parent plan exit criteria status (after Phase D library-side)

The parent plan
(`docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md`)
has eight criteria. After Phase D library-side:

1. ✅ `RenderEffect` is the only effect-enum name; data-only criteria hold. (Phase B)
2. ✅ `EffectElement` and `RenderEffect` in a dedicated module with rustdoc. (Phase B)
3. ✅ `architecture.md §8` and `specification.md §4.4/§5` use `RenderEffect`. (Phase B)
4. ❌ `src/elements/` ≤1,200 production lines or every retained widget has a named consumer. Phase C not started.
5. ⚠️ c4tui placeholders cleared: NavPicker consolidation done (Tasks 1–4); Modal unification, image-viewport winner deletion, LinkDirectory, view-store split all pending.
6. ⏳ Operator sign-off on local + SSH + container smoke tests — **scripted; awaiting operator** (Phase D commit 3 ships the checklist).
7. ✅ testkit verifies render output + render effects without a live terminal. (Phase E)
8. ❌ `architecture.md §2` widget table and prelude re-exports reflect surviving modules only — pending Phase C.

**4 of 8 met, 1 of 8 scripted-awaiting-operator** (up from 4 met before
Phase D).

## What's next for tui-kit

- **Operator step (out-of-band):** run
  `docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md` against
  real terminals + SSH + container layers; record results in the Sign-off
  block; commit. This closes parent-plan exit criterion #6.

- **Phase C — Elements triage.** Still gated on c4tui Phase F Tasks 5–8
  (Modal unification, image-viewport winner deletion, LinkDirectory,
  view-store split) landing — those may pull in `Stack`, `Window`, `Modal`,
  `Overlay`, etc. before C can honestly delete. Until those Tasks land,
  Phase C is on hold by design.

If c4tui Phase F advances meaningfully before the operator runs the smoke
checklist, the next on-path tui-kit work is **Phase C** for whichever
sub-area Phase F has signed off on.

## Cross-repo context

c4tui was unaffected by Phase D in this session. Phase D's tests are
internal to tui-kit; the smoke checklist drives c4tui as the consumer but
does not require c4tui code changes. c4tui's `main` works against the
current tui-kit `main` without modification.

## How to pick up tui-kit work from here

```bash
cd /Users/coleshaffer/Projects/tui-kit
git pull
# Status: Phase B, E, and Phase D library-side complete.
# Operator action needed for Phase D criterion #6 (smoke checklist).
# Phase C remains gated on c4tui Phase F Tasks 5-8.
cat docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md
```

The Phase D plan
(`docs/superpowers/plans/2026-05-15-phase-D-image-reliability.md`) and the
operator smoke checklist
(`docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`) remain in
place alongside the parent operational plan.
