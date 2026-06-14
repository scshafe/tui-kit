# Handoff — Phase 2 complete, Phase 3 ready to fire

**Date:** 2026-05-13
**State:** Phase 2 of the tui-kit + c4tui refactor roadmap is shipped on both repos' `main`. Phase 3 is queued and unstarted.

If you're a fresh Claude Code session reading this — start here. Everything you need to pick up is in this document.

---

## Repos and current state

Both at `/Users/coleshaffer/Projects/`:

| Repo | Role | main HEAD | Tests | clippy | fmt |
|------|------|-----------|-------|--------|-----|
| `tui-kit` | Domain-neutral terminal UI substrate | `3f6ca17` | 154 lib + 11 parity passing | clean | clean |
| `c4tui` | Product consumer of tui-kit (path dep `tui-kit = { path = "../tui-kit" }`) | `f9857df` | 90 passing | clean | clean |

GitHub remotes: `scshafe/tui-kit`, `scshafe/c4tui`. Default branch `main`.

Verify before doing anything else:
```bash
cd /Users/coleshaffer/Projects/tui-kit && cargo test --quiet 2>&1 | grep "test result:" && cargo clippy --all-targets --quiet 2>&1 | tail -3 && cargo fmt --check
cd /Users/coleshaffer/Projects/c4tui && cargo test --quiet 2>&1 | grep "test result:" && cargo clippy --all-targets --quiet 2>&1 | tail -3 && cargo fmt --check
```

---

## The roadmap

`/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`

Seven phases. Each has a self-contained **architect handoff prompt** that produces a concrete task-by-task plan when fired at the `the-architect` subagent.

| Phase | Items | Status |
|-------|-------|--------|
| 1 — tui-kit primitive cleanup | #3, #9, #10 | ✅ SHIPPED on main |
| 2 — Input event boundary cleanup | #7 | ✅ SHIPPED on main |
| 3 — NavPicker + modal-slot + image-widget winner + elements decision | #1, #2, #4, #5 | ⏳ UP NEXT (centerpiece) |
| 4 — Command + Effect cleanup | #8, #11 | ⏳ |
| 5 — LinkDirectory implementation | #6 | ⏳ goal of the whole sequence |
| 6 — ViewStore split | #12 | ⏳ |
| 7 — Documentation + retention tooling | #13, #14, #15, #16, #17 | ⏳ |

---

## What Phase 2 actually delivered

**tui-kit main (8 commits since Phase 1's tip, oldest first):**
```
3be9756  split input::Key into KeyEvent + MouseEvent + InputEvent
51b1531  filter unmapped mouse events to None instead of synthetic Release
300eb2f  rename keymap consumers to KeyEvent
eed7b51  keep SpecialKey::from_key_event private
ad0d410  rename Grid key consumers to KeyEvent
ab1b261  rename Element event type to KeyEvent
8558409  route AppEvent::Input through unified input::InputEvent
3f6ca17  apply cargo fmt to Phase 2 changes
```

**c4tui main (2 commits since Phase 1's tip):**
```
f7201e8  Rename Key consumers to KeyEvent and drop translate_key indirection
f9857df  Absorb mouse events in modal scopes and apply cargo fmt
```

**End-state facts a future plan can assume:**
- `tui_kit::input::{KeyEvent, MouseEvent, InputEvent}` are the three input types. The old `Key` enum is gone everywhere.
- `MouseEvent` carries terminal-cell coordinates (1-indexed). Conversion to canvas fractions is **not** a method on `MouseEvent` — that conversion lives in c4tui.
- `tui_kit::events::InputEvent` (the AppEvent-category wrapper) is **deleted**. `AppEvent::Input` now carries `tui_kit::input::InputEvent` directly.
- `tui_kit::input_thread::spawn` demuxes `InputEvent::Resize` into `AppEvent::Terminal(TerminalEvent::Resize)`, preserving c4tui's resize-coalescing path. All other input flows through `AppEvent::Input`.
- `crate::events::TerminalEvent::Resize` remains; `InputEvent::Resize` and `TerminalEvent::Resize` carry identical data — the duplication is intentional (different delivery paths). Minor smell, not a bug.
- `tui_kit::input::translate_mouse` returns `Option<MouseEvent>` — unmapped mouse events (right-click, middle button, etc.) become `None` and are filtered by `read_input_event`'s skip-and-retry loop. This is a deliberate improvement over the plan's original "synthetic Release fallback" idea.
- c4tui's parallel `event::InputEvent` enum is **deleted**.
- `c4tui::event::mouse_to_canvas_fraction(MouseEvent, CanvasMetrics, status_rows) -> Option<(f32, f32)>` is the **single** boundary function where terminal-cell coords become canvas fractions for click/wheel events. Drag coords still travel as cells through `Command::DragTo` — explicit Phase 4 tech debt.
- `TerminalBackend::translate_key` is **deleted** from both the trait and `FakeTerminalBackend` / `TerminalSession` impls. `c4tui::App::handle_input(InputEvent)` is the universal non-modal entry point.
- `c4tui::App` now has `handle_input_event(InputEvent)` (top-level dispatcher) plus `fn handle_key_event(KeyEvent)` (modal-aware keyboard router, used by the dispatcher and by tests). Modal scopes (`SCOPE_PICKER`, `SCOPE_CONNECTION_PICKER`, `SCOPE_LOG`, `SCOPE_DIALOG`) **absorb** mouse events as no-ops — preserves pre-rename behaviour where modal `_ => Continue` arms silently dropped mouse clicks.
- `Element<Message=M>` subtrait bound changed from `BufferComponent<Event=Key>` to `BufferComponent<Event=KeyEvent>`.

---

## No force-push drama this time

Unlike Phase 1, there was no parallel-agent divergence. Both repos' local `main` matched `origin/main` at fast-forward time, and `git push origin main` advanced cleanly:
- tui-kit: `471b38e` → `3f6ca17` (fast-forward)
- c4tui: `2c2f4c1` → `f9857df` (fast-forward)

No reflog rescue needed.

---

## Outstanding smells (small)

Two known minor smells worth tracking but not worth interrupting Phase 3 to fix:

1. **`#[allow(unused_imports)]` on `c4tui::event::TuiKitInputEvent` re-export.** The plan introduced `pub use tui_kit::input::InputEvent as TuiKitInputEvent;` in `c4tui/src/event.rs:171` as a "single place to look for the input event type" for c4tui modules. But the plan's `app.rs` step instructed importing `InputEvent` directly from `tui_kit::input`, so the alias has zero current consumers. The implementer suppressed the resulting warning rather than delete the alias. **Phase 3 should decide:** wire `app.rs` (and NavPicker) through the alias, or delete the alias entirely.

2. **Vestigial `InputEvent::Resize { .. }` arm in `c4tui::App::handle_input_event`** (`app.rs:302`). Unreachable in production because the input thread already demuxes `Resize` into `AppEvent::Terminal`. Kept to make the `match` exhaustive. The duplication between `InputEvent::Resize` and `TerminalEvent::Resize` carrying identical data is structural; consolidation is Phase 4-territory if anything.

**Plus one test gap:** the modal mouse-absorption fix in `app.rs:290-301` is uncovered. No test clicks while a picker scope is active and asserts the click is dropped. Phase 4 should add one when it touches command dispatch — adding it now would be drive-by scope creep.

---

## Phase 4 tech debt captured (informational — for Phase 4's planner, not Phase 3's)

1. **`Command::DragTo { x: u16, y: u16, canvas: CanvasMetrics }`** at `c4tui/src/event.rs:155` carries raw cell coords + bundled canvas. `state.rs:191-206` recomputes fractions from `last_drag` against `drag_canvas`. This is the **only** remaining place cell coords cross into the command layer. Phase 4 should fold this into `mouse_to_canvas_fraction` so `Command::DragTo` carries `dx_fraction/dy_fraction` like every other navigation command.
2. **`PendingCommand` → `Command` resolution** (`c4tui/src/event.rs:71-114`) is almost an identity map. Phase 4's collapse should eliminate the indirection. Note: click/wheel resolution paths already pass canvas through `mouse_to_canvas_fraction` and emit fraction-bearing variants directly — only `DragTo` is the outlier above.
3. **`STATUS_ROWS = 1` is duplicated** in `c4tui/src/keymap.rs:21` and `c4tui/src/terminal.rs:24`. If/when the status bar becomes configurable per `AppConfig`, both call sites need to read from the config rather than a constant.
4. **`Resize` shape duplication** (`InputEvent::Resize` and `TerminalEvent::Resize`) — see "Outstanding smells" above. Low priority.

---

## In-flight corrections during Phase 2 (pattern likely to repeat)

The architect's Phase 2 plan was strong but, like Phase 1's, had small inaccuracies caught and fixed during execution:

1. **Task 2 — `translate_mouse` fallback was a behavioral regression.** Plan had unmapped mouse events collapsing into a synthetic `MouseEvent::Release`, which would have collided with real button-up events at the inner mouse level. Code-quality reviewer caught it; we changed `translate_mouse` to return `Option<MouseEvent>` and filter `None` in `translate_event` (symmetric with how `translate_key_event` already handles key-release events).
2. **Task 3 — plan literal had `pub fn from_key_event` but the original was `fn`.** Transcription error. We reverted to private (no external callers in either repo).
3. **Task 7 — modal-scope mouse-event regression.** Plan's `handle_input_event` routed all mouse events straight to the root handler regardless of scope. Pre-rename, modal handlers silently absorbed mouse clicks via `_ => Continue` arms. Code-quality reviewer caught the regression; we routed mouse events to the root handler **only** when `active_scope() == SCOPE_ROOT`, and made modal scopes drop them.
4. **Task 7 — `TuiKitInputEvent` alias has no consumer.** Plan's literal Step 8(a) directed `app.rs` to import `InputEvent` directly from `tui_kit::input`, conflicting with the plan's stated purpose of the alias ("one place for c4tui modules to look"). Implementer suppressed the warning; tagged as a Phase 3 cleanup. (See Outstanding smells #1.)
5. **Task 7 — `cargo fmt` drift introduced by the rename.** `Key` → `KeyEvent` pushed several lines past the 100-col rustfmt width in c4tui. Cleaned up in the final `Apply cargo fmt` commit; same pattern as Phase 1's `Apply cargo fmt to elements.rs` commit.

Future phases should expect more of the same. The two-stage review (spec compliance then code quality) caught every meaningful regression that slipped past the implementer's self-review.

---

## To resume — concrete next actions

1. **Verify clean baseline.** Run the cargo test + clippy + fmt check in both repos (commands at the top of this doc).
2. **Fire the Phase 3 architect prompt.** From `docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`, copy the Phase 3 architect handoff prompt verbatim and dispatch:
   ```
   Agent tool, subagent_type: "the-architect"
   prompt: <the verbatim Phase 3 handoff prompt from the roadmap>
   ```
3. **When the architect returns a plan,** offer the user the same execution choice as Phases 1 and 2: subagent-driven-development with two-stage reviews. If they accept, follow the same workflow.
4. **In Phase 3's plan, expect the architect to address:** the `TuiKitInputEvent` alias decision (use or delete), Phase 3's mouse-event interaction with the new NavPicker (does NavPicker want mouse?), the modal mouse-absorption test gap (likely fold into NavPicker's test suite), and the updated direction that preserves `elements` as tui-kit's render-effect substrate.

---

## Phase 3 scope (so you don't have to dig)

Phase 3 lands items **#1, #2, #4, #5** from the architecture review. It is the centerpiece of the whole roadmap:

- **#1 NavPicker:** Unify `ViewPicker` and `ConnectionPicker` (and the future LinkDirectory picker) into a single c4tui component built on `Grid`. Should consume `KeyEvent` (and possibly `MouseEvent` — open question for the architect).
- **#2 elements decision:** Preserve `elements` as the render-effect substrate. Do not validate it by porting NavPicker through it, and do not delete it in Phase 3. Later renderer-backend work can shrink/rename the effect model deliberately.
- **#4 image-widget winner:** Pick between `ImageBox` and `ImageViewport` — the two image widgets in tui-kit's `widgets/`. The architect should evaluate which one survives based on actual c4tui consumers.
- **#5 modal-slot:** Either solidify or delete the modal-slot abstraction in c4tui. Architect's call.

The full self-contained prompt is in the roadmap; don't paraphrase it.

---

## File paths cheat sheet (unchanged from Phase 1)

- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md` — the roadmap, now 9 phases after the remote-rendering direction update
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-1-tui-kit-primitive-cleanup.md` — Phase 1's plan
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-2-input-event-boundary.md` — Phase 2's plan
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/handoffs/2026-05-12-phase-1-complete.md` — Phase 1 handoff
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/handoffs/2026-05-13-phase-2-complete.md` — this doc
- `/Users/coleshaffer/Projects/tui-kit/specification.md` — tui-kit's domain-neutral spec
- `/Users/coleshaffer/Projects/tui-kit/architecture.md` — tui-kit's architecture
- `/Users/coleshaffer/Projects/tui-kit/README.md` — tui-kit positioning
- `/Users/coleshaffer/Projects/c4tui/specification.md` — c4tui keyboard-first nav direction
- `/Users/coleshaffer/Projects/c4tui/architecture.md` — c4tui structure + LinkDirectory sketch
- `/Users/coleshaffer/Projects/c4tui/README.md` — c4tui positioning

---

## If you take only one thing from this handoff

Same as Phase 1's: the user values **terse responses, fast execution, no busywork, no commentary, real testing, real review.** The two-stage review workflow earned its keep again this phase — every meaningful regression slipped past the implementer's self-review and was caught by the second-stage code-quality reviewer. Don't skip the reviews. Don't reject reviewer deviations reflexively, but don't accept them reflexively either — adjudicate each one. Match the user's disposition: act decisively, but never silently let a quality regression through.
