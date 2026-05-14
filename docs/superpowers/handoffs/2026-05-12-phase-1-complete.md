# Handoff — Phase 1 complete, Phase 2 ready to fire

**Date:** 2026-05-12
**State:** Phase 1 of the tui-kit + c4tui refactor roadmap is shipped on both repos' `main`. Phase 2 is queued and unstarted.

If you're a fresh Claude Code session reading this — start here. Everything you need to pick up is in this document.

---

## Repos and current state

Both at `/Users/coleshaffer/Projects/`:

| Repo | Role | main HEAD | Tests | fmt |
|------|------|-----------|-------|-----|
| `tui-kit` | Domain-neutral terminal UI substrate | `471b38e` | 152 lib + 11 parity passing | clean |
| `c4tui` | Product consumer of tui-kit (path dep `tui-kit = { path = "../tui-kit" }`) | `2c2f4c1` | 90 passing | clean |

GitHub remotes: `scshafe/tui-kit`, `scshafe/c4tui`. Default branch `main`.

Verify before doing anything else:
```bash
cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | grep "test result:" && cargo fmt --check
cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | grep "test result:" && cargo fmt --check
```

---

## The roadmap

`/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`

Seven phases derived from a 2026-05-12 architecture review of both repos. Each phase contains a self-contained **architect handoff prompt** that produces a concrete task-by-task plan when fired at the `the-architect` subagent.

| Phase | Items | Status |
|-------|-------|--------|
| 1 — tui-kit primitive cleanup | #3, #9, #10 | ✅ SHIPPED on main |
| 2 — Input event boundary cleanup | #7 | ⏳ UP NEXT |
| 3 — NavPicker + modal-slot + image-widget winner + elements decision | #1, #2, #4, #5 | ⏳ centerpiece |
| 4 — Command + Effect cleanup | #8, #11 | ⏳ |
| 5 — LinkDirectory implementation | #6 | ⏳ goal of the whole sequence |
| 6 — ViewStore split | #12 | ⏳ |
| 7 — Documentation + retention tooling | #13, #14, #15, #16, #17 | ⏳ |

Items #1–#17 are defined in the roadmap's appendix priority table.

---

## What Phase 1 actually delivered

**tui-kit main (8 commits, oldest first):**
```
c7c12bb  Reposition tui-kit as domain-neutral substrate
375ef39  Add refactor roadmap and Phase 1 plan
0e76ea5  add forwarding BufferComponent impl for boxed trait objects
401d6cc  drop frame-based Component trait in favor of BufferComponent
511253e  collapse Element trait into BufferComponent<Event=Key> subtrait
b2b6225  reduce ImageProtocol to { Kitty, Noop } and drop unreachable validation paths
0b4e118  slim prelude to constructors and traits
471b38e  Apply cargo fmt to elements.rs
```

**c4tui main (relevant Phase 1 commit on top of doc reframe):**
```
ddc0d3d  Reframe spec and architecture for keyboard-first navigation
2c2f4c1  drop dead Component import after tui-kit trait collapse
```

**End-state facts a future plan can assume:**
- One rendering trait: `BufferComponent`. The Frame-based `Component` trait is gone.
- `Element<Message=M>` is a marker subtrait of `BufferComponent<Event=Key>` via blanket impl. `pub trait Element: BufferComponent<Event = Key> {}` + `impl<T> Element for T where T: BufferComponent<Event = Key> {}` at `src/elements.rs:37-39`.
- `BufferComponent::handle_event` has a default returning `Ok(ComponentOutcome::Ignored)`. Impls only override when they have real event-handling work.
- `Cached<C>` has inherent `handle_event` (delegates to inner without invalidating) and inherent `inner_mut` (invalidates cache).
- `ImageProtocol` is `{ Kitty, Noop }`. `unsupported_protocol_error` and `image_protocol_is_implemented` are gone. `explicit_noop_error` remains.
- `src/prelude.rs` re-exports constructors + traits only. Config/placement/error/internal-state types reached via module paths.
- c4tui's `app.rs` modal handlers call `slot.picker.handle_event(&key)` (the inherent method on `Cached`, added in Phase 1).

---

## ⚠️ Critical context: the force-push

While Phase 1 was executing in this session, **another agent was working on both repos' main branches in parallel**. By the time Phase 1's PRs were ready to merge:

- tui-kit's `origin/main` had 20 new commits with wholesale deletions: `src/elements.rs` (3,763 lines), `src/widgets/grid.rs` (1,047 lines), `src/widgets/image_box.rs` (907 lines), `src/widgets/image_viewport.rs` (1,410 lines), and all of `visual-tests/`. Net diff: ~11k lines deleted, ~1.2k added.
- c4tui's `origin/main` had 16 commits, mostly test additions and tui-kit lockfile refreshes. Notable: `bcc303c ci: consume tui-kit from git` switched c4tui from path dep to git dep (the path dep is now restored, see below).

The user **explicitly authorized overwriting both repos' main branches** with our Phase 1 work via `git push --force-with-lease`. Phase 1's content is now on main; the other agent's work is no longer reachable from main.

The deleted commits are still recoverable from the git reflog and from the GitHub event log (the force-push event is visible in `https://github.com/scshafe/tui-kit/activity` etc.). If a future session needs them, they exist.

**Important open question.** The deletions on the overwritten origin/main looked like a parallel implementation of items in this same roadmap (specifically the "delete speculative surfaces without consumers" rule that #2, #4, and parts of #9 prescribe). Before starting Phase 2, **ask the user** whether they want to:
1. Look at the deleted main to see if there's salvageable work, OR
2. Proceed with Phase 2 as planned, accepting that the other agent's work is gone.

If proceeding with (2), be aware that the user may have another automation running on main that will *redo* those deletions. If they want to keep that automation off during Phase 2 execution, that's worth raising.

---

## Execution model

- The roadmap document is durable. Per-phase plans are generated on demand by `the-architect`.
- For each phase:
  1. Find that phase's "Architect handoff prompt" block in the roadmap.
  2. Copy it verbatim. Dispatch via `Agent` tool with `subagent_type: "the-architect"`.
  3. The architect produces `docs/superpowers/plans/2026-05-12-phase-N-<name>.md` in the writing-plans style (TDD, exact paths, complete code in steps, bite-sized tasks, frequent commits).
  4. Execute that plan via `superpowers:subagent-driven-development` — fresh subagent per task, two-stage review (spec compliance, then code quality).
- Hard user constraints across all phases:
  - **No backwards-compatibility shims.** No deprecated re-exports. No "kept for migration" comments. Rip and replace.
  - **Both repos edited atomically.** If a tui-kit change breaks c4tui imports, fix the c4tui imports in the same task.
  - **End state, not graceful transition.** Big diffs are fine. The user explicitly chose subagent-driven-development with two-stage reviews precisely so quality regressions get caught — don't skip the reviews.

---

## In-flight corrections during Phase 1 (pattern likely to repeat)

The plan generated by the architect was strong but not perfect. Five things were caught and fixed mid-execution:

1. **Task 2 — TDD test wasn't actually witnessing the impl.** Rust's auto-deref through `Box<dyn Trait>` routes method calls without an explicit impl, so the original failing-test step didn't fail. Strengthened with a trait-bound assertion.
2. **Task 3 — Plan understated c4tui's blast radius.** Said c4tui was affected only by the import. False — two production call sites in `app.rs` used the deleted `Cached::handle_event` trait method. Caught and fixed atomically.
3. **Task 3 follow-up — First fix had a perf regression.** Rewriting call sites as `inner_mut().handle_event()` added cache invalidation on every keystroke. Code-quality review caught it; we added `Cached::handle_event` as an inherent method instead.
4. **Tasks 4+5 — Plan didn't anticipate four impls using the trait default.** Added the default to `BufferComponent::handle_event` instead of writing four explicit `Ok(Ignored)` overrides.
5. **Task 9 — Plan put `ImageViewportElement` in the wrong module.** It lives in `crate::elements`, not `crate::widgets::image_viewport`.

Future phases should expect similar small plan inaccuracies. The right response is to call them out, decide on the spot whether the implementer's fix is correct, and let the review loops catch anything that slips through. Don't reject deviations reflexively.

---

## To resume — concrete next actions

1. **Verify clean baseline.** Run the cargo test + fmt check in both repos (commands at the top of this doc).
2. **Decide on the overwritten-main question.** Ask the user whether they want to look at the deleted work first, or proceed.
3. **Fire the Phase 2 architect prompt.** From `docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`, copy the Phase 2 architect handoff prompt verbatim and dispatch:
   ```
   Agent tool, subagent_type: "the-architect"
   prompt: <the verbatim Phase 2 handoff prompt from the roadmap>
   ```
4. **When the architect returns a plan,** read it briefly and offer the user the same execution choice as Phase 1: subagent-driven-development with two-stage reviews. If they accept, follow the same workflow.

---

## Phase 2 scope (so you don't have to dig)

Phase 2 lands item **#7** from the architecture review: split tui-kit's `input::Key` into three types — `KeyEvent` (keyboard), `MouseEvent` (click/drag/wheel/release), `InputEvent` (the union plus `Resize`) — and collapse c4tui's parallel `event::InputEvent` into tui-kit's. The full self-contained prompt is in the roadmap; don't paraphrase it.

Why this phase before Phase 3: NavPicker (Phase 3's centerpiece) will receive input events through the new shape, so doing the split first means NavPicker lands with the new event types from day one.

---

## File paths cheat sheet

- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md` — the roadmap, all 7 phases
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-1-tui-kit-primitive-cleanup.md` — Phase 1's executed plan, kept for reference
- `/Users/coleshaffer/Projects/tui-kit/specification.md` — tui-kit's domain-neutral spec
- `/Users/coleshaffer/Projects/tui-kit/architecture.md` — tui-kit's architecture
- `/Users/coleshaffer/Projects/tui-kit/README.md` — tui-kit positioning
- `/Users/coleshaffer/Projects/c4tui/specification.md` — c4tui keyboard-first nav direction
- `/Users/coleshaffer/Projects/c4tui/architecture.md` — c4tui structure + LinkDirectory sketch
- `/Users/coleshaffer/Projects/c4tui/README.md` — c4tui positioning

---

## If you take only one thing from this handoff

The user values: **terse responses, fast execution, no busywork, no commentary, real testing, real review.** They explicitly chose two-stage review per task so problems get caught. They explicitly authorized destructive force-push when the situation called for it. Match that disposition: act decisively, but never silently let a quality regression through.
