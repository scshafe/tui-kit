# Handoff — tui-kit Phase B (RenderEffect Contract) complete

**Date:** 2026-05-14
**Repo:** tui-kit
**Scope:** This handoff covers tui-kit only. For the c4tui side of the parallel work, see
`/Users/coleshaffer/Projects/c4tui/docs/superpowers/handoffs/2026-05-14-phase-F-navpicker-consolidation-complete.md`.

## State

| Repo | Branch | HEAD | Tests | clippy | fmt | Origin |
|------|--------|------|-------|--------|-----|--------|
| `tui-kit` | `main` | `ce9c0b2` | 155 lib + 11 parity passing | clean | clean | pushed |

Verify locally:

```bash
cd /Users/coleshaffer/Projects/tui-kit
cargo test --quiet 2>&1 | grep "test result:"
cargo clippy --all-targets --quiet
cargo fmt --check
```

## What shipped in this session

This session covered Phase A (direction pivot + doc alignment) plus all of Phase B (the RenderEffect contract).

| Commit | Subject |
|--------|---------|
| `7d47d59` | docs(plans): pivot to library-author direction; supersede older roadmap |
| `2e33ae2` | docs: align core docs with render-effect/renderer-backend direction |
| `049e418` | docs(plans): track historical phase plans and completion handoffs |
| `e7b5f6f` | docs(plans): tighten fresh-library-author-plan after Phase A review |
| `125e6ab` | docs(plans): add revised library-author implementation plan |
| `dc68f6c` | refactor(elements): extract TerminalEffect and EffectElement to effect.rs |
| `ce9c0b2` | refactor(elements): rename TerminalEffect to RenderEffect; lock data-only contract |

### Phase B details (the load-bearing tui-kit change)

Phase B was specified as two commits in the operational plan and landed exactly that way:

1. **`dc68f6c` — extract.** Moved `TerminalEffect`, `EffectElement`, and the apply methods from the 3,727-line `src/elements.rs` into `src/elements/effect.rs`. Converted `src/elements.rs` to `src/elements/mod.rs`. Behavior-preserving move, no rename, no consumer impact.
2. **`ce9c0b2` — rename + lock contract.** Renamed `TerminalEffect → RenderEffect` and `terminal_effects() → render_effects()`. Added module-level rustdoc on `src/elements/effect.rs` enumerating the five data-only constraints (derives `Clone + Debug + PartialEq + Eq`; no `Box<dyn _>`; no `Fn`/closure fields; no `Arc<Mutex<_>>` live-state handles; no ambient terminal/app-state access). Added `tests::render_effects_are_data_only` as a structural round-trip test that fails to compile if a future variant violates the contract. Updated README's one stale reference. `architecture.md §8` and `specification.md §4.4/§5` were already aligned in Phase A.

The data-only contract is now load-bearing: the structural test will refuse to compile any variant that breaks it.

## Plan references

- **Strategic direction:** `docs/superpowers/plans/2026-05-13-library-author-direction.md`
- **Fresh forward plan (strategic):** `docs/superpowers/plans/2026-05-13-fresh-library-author-plan.md`
- **Operational plan (this is the doc executors should read first):** `docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md`
- **Older joint refactor roadmap (superseded for forward planning, historical):** `docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`

## Plan-level Exit criteria (operational plan)

Eight criteria for tui-kit being "library-author-ready":

1. ✅ `RenderEffect` is the only effect-enum name; data-only criteria hold.
2. ✅ `EffectElement` and `RenderEffect` in a dedicated module with rustdoc stating the data-only contract.
3. ✅ `architecture.md §8` and `specification.md §4.4/§5` use `RenderEffect` consistently.
4. ❌ `src/elements/` ≤1,200 production lines or every retained widget has a named consumer — currently ~3,600 lines. Phase C not started.
5. ⚠️ c4tui placeholders cleared: NavPicker consolidation done (Tasks 1–4); Modal unification, image-viewport winner deletion, LinkDirectory, view-store split all pending.
6. ❌ Operator sign-off on local + SSH + container smoke tests — Phase D not started.
7. ❌ testkit verifies render output + render effects without a live terminal — Phase E not started.
8. ❌ `architecture.md §2` widget table and prelude re-exports reflect surviving modules only — pending Phase C.

3 of 8 met.

## What's next for tui-kit

The diamond dependency graph in the operational plan is: `A (shipped) → {B (shipped), F (in progress in c4tui)} → C, B → {D, E}`.

Phases unblocked, in order of natural pickup:

- **Phase D — Image path + SSH/container reliability** (library-side work is library-internal: tests for image lifecycle through `MockImageSurface`, env-var documentation; operator owns the live smoke tests). Independent of c4tui's Phase F progress.
- **Phase E — Testkit hardening** (helper assertions for `RenderEffect` sequences; pure buffer rendering ergonomics; docs on testkit vs c4tui fake backend). Independent of Phase F.
- **Phase C — Elements triage** (deletion-by-default of retained widgets c4tui doesn't consume). Gated on c4tui Phase F reports per-sub-area. As of this handoff, c4tui's Phase F Tasks 1–4 (NavPicker consolidation) have confirmed that `Panel`, `Stack`, `Window`, `Modal`, `Overlay`, `WindowChrome`, `KeyScope`, `ImageViewportElement` are still unconsumed. The first wave of Phase C deletions can begin once Tasks 5–8 of c4tui's Phase F land (Modal unification, image-viewport winner deletion, LinkDirectory, view-store split) — those may pull some types back.

## Cross-repo context

In parallel with Phase B, c4tui's `main` advanced from `f9857df` to `386a3d3` — 9 commits implementing Phase F Tasks 1–4 (NavPicker consolidation). c4tui no longer references `TerminalEffect` (never did) or `RenderEffect` directly; the rename had zero c4tui impact. See c4tui's Phase F handoff for the consumer-side narrative.

## How to pick up tui-kit work from here

```bash
cd /Users/coleshaffer/Projects/tui-kit
git pull
# Read the operational plan:
cat docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md
# Choose Phase D or Phase E to start (they're independent and library-internal).
```

The operational plan's per-phase sections include Dependencies / Scope / Exit / Non-goals / Risks. Treat them as the canonical spec; per-task bite-sized plans can be written when execution starts (the writing-plans skill applies).
