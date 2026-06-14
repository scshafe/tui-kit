# Handoff — tui-kit `widgets::image_box` deletion complete

**Date:** 2026-05-15
**Repo:** tui-kit
**Scope:** This handoff covers the cross-repo Phase 3 plan Task 8 deletion,
landed in tui-kit. c4tui was verified compatible; no c4tui changes were
needed.

## State

| Repo | Branch | HEAD | Tests | clippy | fmt | Origin |
|------|--------|------|-------|--------|-----|--------|
| `tui-kit` | `main` | `15c41ab` | 153 lib + 11 parity passing | clean | clean | pushed |
| `c4tui` | `main` | `cd32717` | 95 passing | (unchanged) | (unchanged) | (unchanged) |

Verify locally:

```bash
cd /Users/coleshaffer/Projects/tui-kit
cargo test --quiet 2>&1 | grep "test result:"
cargo clippy --all-targets --quiet
cargo fmt --check

cd /Users/coleshaffer/Projects/c4tui
cargo clean -p c4tui && cargo test --quiet 2>&1 | grep "test result:"
```

## What shipped

One commit (`15c41ab`) deletes `tui-kit/src/widgets/image_box.rs` (907 lines
production + tests) plus the standalone `visual-tests/` crate that was a
manual visual test for `ImageBox`, and reconciles the prelude, the
widget-table in README, the spec, and the README's full `ImageBox` section.

Net: 12 files changed, 8 insertions, 1833 deletions.

## Why this was unblocked

The cross-repo Phase 3 plan Task 8 (which lives in c4tui as the consumer
plan, `c4tui/docs/superpowers/plans/2026-05-12-phase-3-navpicker-modal-image-elements.md`)
states explicitly:

> This is a subtractive task with no logical dependency on Tasks 1-7. Doing
> it after the new modal infrastructure is solid keeps the high-risk and
> low-risk work separate.

c4tui consumes none of `ImageBox`, `ImageBoxPlan`, `ImageBoxState`
(grep-verified zero hits across `c4tui/src/`). The Phase B handoff already
confirmed `image_viewport` is the consumer-validated survivor and
`image_box` had no downstream consumer.

The deletion is the **first wave of tui-kit Phase C** — it advances parent-
plan exit criterion #4 ("`src/elements/` ≤1,200 production lines or every
retained widget has a named consumer"). `image_box` lived in `widgets/`,
not `elements/`, so it doesn't directly shrink `elements/`, but it
demonstrates the Phase C deletion discipline on a fully-confirmed target.

## Parent plan exit criteria status (after this deletion)

The parent plan
(`docs/superpowers/plans/2026-05-14-revised-library-author-implementation-plan.md`)
has eight criteria. Unchanged numerically (still 4 of 8 met + 1 scripted-
awaiting-operator) — `image_box` deletion is c4tui-plan Task 8, not a
parent-plan exit criterion in itself. But it lines up two adjacent shifts:

- Exit criterion #5 ("c4tui placeholders cleared") moves materially closer
  as one of the four pending c4tui Phase F sub-tasks is now closed (the
  image-viewport winner deletion).
- The Phase C deletion discipline is now demonstrated on a real target.

## What's next for tui-kit

- **Operator step (still pending):** run the smoke checklist at
  `docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`, record
  results, close parent criterion #6.

- **Phase C continuation** — still gated on c4tui Phase F Tasks 5–7 for the
  retained widgets in `src/elements/mod.rs` (`Panel`, `Stack`, `Window`,
  `Modal`, `Overlay`, `WindowChrome`, `KeyScope`, `ImageViewportElement`).
  Until c4tui's Modal-trait + ActiveModal work lands (Phase F Task 5), tui-
  kit cannot honestly delete `Modal`, `Overlay`, etc. — they may be
  consumed.

- **The next unblocked piece on the project ledger** is c4tui Phase F Task 5
  (Modal trait + ActiveModal). Detailed plan: `c4tui/docs/superpowers/plans/2026-05-12-phase-3-navpicker-modal-image-elements.md`
  line 1386 onward. Substantial: ~258 plan lines.

## Cross-repo context

c4tui was unaffected functionally by this deletion. `cargo clean -p c4tui &&
cargo test` against the new tui-kit `main` succeeds with 95 tests passing.

## Local cleanup the operator can do

The `visual-tests/` directory still contains untracked artifacts: a build
target (`target/`), Python cache (`scripts/__pycache__/`), one archived PNG
fixture (`archived/pngs/image_box_source_fixture.png`), and `.DS_Store`
files. None are tracked by git. The deletion left these in place because
`rm -rf` is destructive — the operator can clean up with `rm -rf visual-
tests` at their convenience.
