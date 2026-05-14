# tui-kit + c4tui Refactor Roadmap

> **Status:** SUPERSEDED for forward planning. Use
> [`2026-05-13-fresh-library-author-plan.md`](./2026-05-13-fresh-library-author-plan.md)
> and
> [`2026-05-13-library-author-direction.md`](./2026-05-13-library-author-direction.md)
> as the active direction. This document remains as historical context for the
> 2026-05-12 refactor sequence.

> **For agentic workers:** This is a phased *roadmap*, not a task-by-task plan. Each phase contains an **Architect handoff prompt** block. When you're ready to execute a phase, fire that prompt at the `the-architect` agent (subagent_type: `the-architect`) to produce the detailed task-by-task plan in the standard writing-plans style. Then execute the resulting per-phase plan via `superpowers:subagent-driven-development`.

**Goal:** Execute the seventeen refactors from the 2026-05-12 architecture review of tui-kit and c4tui, in dependency order, terminating in a clean implementation of the `LinkDirectory` keyboard-first navigation model, while preserving tui-kit's longer-term path toward local and remote renderer backends.

**Operating constraint (set by user):** *No backwards-compatibility shims, no deprecation periods, no migration layers, no preserving old names "for a release."* Rip-and-replace at the source. We're optimizing for end state, not graceful transition. Per-phase architect plans must assume both repos can be edited atomically and that downstream breakage inside the same workspace is fine as long as the final state of the phase compiles and tests pass.

**Source review:** The 17 numbered topics referenced in this roadmap come from the architect's review delivered 2026-05-12. The priority table is at the end of this document for reference.

**Repos:**
- tui-kit: `/Users/coleshaffer/Projects/tui-kit` (domain-neutral terminal UI substrate)
- c4tui: `/Users/coleshaffer/Projects/c4tui` (consumer of tui-kit; product surface)

## Directional update — local now, remote-capable later

After the initial Phase 3 planning, the target direction expanded: tui-kit
should remain a terminal-first substrate, but its rendering boundaries should
also support a future client-side renderer for apps running over SSH. That
future is **not** Phase 3 work. It changes the evaluation of `elements`: the
useful concept is not a speculative UI DSL, but a composable buffer-rendering
tree that can emit explicit render effects such as image upload, placement,
teardown, and cleanup.

The consequence for the roadmap:

- Do not delete `elements` merely because `NavPicker` does not need it.
- Do not port `NavPicker` through `elements`; pickers remain app-owned
  `Grid`/ratatui components.
- Preserve and later reshape the render-effect nucleus: `EffectElement`,
  `TerminalEffect`/future `RenderEffect`, area-transforming containers, and
  image viewport effect wrappers.
- Defer SSH/client/runtime work until the current c4tui refactor sequence has
  produced cleaner modal, command, link-directory, and view-store boundaries.

---

## Phasing summary

| Phase | Title | Items | Repos | Risk |
|-------|-------|-------|-------|------|
| 1 | tui-kit primitive cleanup | #3, #9, #10 | tui-kit | Low |
| 2 | Input event boundary cleanup | #7 | both | Medium |
| 3 | NavPicker + modal-slot + image-widget + elements decision | #1, #2, #4, #5 | both | High (centerpiece) |
| 4 | Command + Effect cleanup | #8, #11 | c4tui | Low-medium |
| 5 | LinkDirectory implementation | #6 | c4tui | Medium |
| 6 | ViewStore split | #12 | c4tui | Medium |
| 7 | Documentation + retention tooling + render-effect docs | #13, #14, #15, #16, #17 | both | Low (doc-heavy) |
| 8 | RenderEffect + renderer backend model | New direction | tui-kit | Medium |
| 9 | `tui-kit-cli` remote launcher prototype | New direction | tui-kit | High |

All 17 items from the review are accounted for across Phases 1-7. Phases 8-9
are new directional phases for remote-capable rendering and are intentionally
sequenced after the c4tui cleanup work.

---

## Phase 1 — tui-kit primitive cleanup

### Scope

Low-risk, mostly tui-kit-internal cleanups that remove duplication and dead surfaces before any downstream work touches the boundary.

### Items covered

- **#3** — Collapse `Component` / `BufferComponent` / `Element` into one trait. After the merge and Phase 2 input split, `Element<Message=M>` is `BufferComponent<Event=KeyEvent, Message=M>`. Drop the Frame-based `Component`; call `frame.buffer_mut()` at use sites.
- **#9** — Slim `prelude.rs` to constructors + traits. Move config types (`ImageBoxPlacement`, `ImageViewportPlacement`, `ImageViewportError`, `ScaledPixelOffset`, `UnscaledPixelOffset`, all the basis/policy enums, etc.) behind module paths. Glob-imports become safe again.
- **#10** — Reduce `ImageProtocol` from `{ Kitty, Sixel, ITerm2, Noop }` to `{ Kitty, Noop }`. Sixel/iTerm2 re-added alongside real consumers if/when those arrive.

### Why this phase first

These three are cheap, independent of the rest of the work, and shrink the public surface that every later phase has to reason about. #3 in particular is a prerequisite for Phase 3 — once there's one trait, the picker/element/window unification has only one type discipline to honor.

### End state of phase

- One trait in `src/component.rs` (or wherever it lands); `elements::Element` is a type alias / parameterization, not a separate trait. The `elements` module itself remains in play for the render-effect path and is preserved in Phase 3.
- Prelude is half its current size; config/error types are reached via `tui_kit::image_box::Placement` etc.
- `ImageProtocol` has two variants; `ImageBackendPreference::Explicit(Sixel)` no longer compiles (it gets deleted).
- All tui-kit tests pass; c4tui builds against the new tui-kit (may require small import-path follow-ups in c4tui, which this phase is allowed to do).

### Architect handoff prompt

```text
You're being asked to expand Phase 1 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan in the writing-plans style
(TDD where applicable, exact file paths, complete code in every step, no
placeholders, bite-sized steps, frequent commits).

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md

Phase 1 scope — three items from the 2026-05-12 architecture review:

  #3 — Collapse Component / BufferComponent / Element into a single trait.
       After the merge and Phase 2 input split, `elements::Element<Message=M>`
       is precisely `BufferComponent<Event=KeyEvent, Message=M>`. Drop the Frame-based
       `Component` trait entirely; call sites do `frame.buffer_mut()`. Keep
       `Cached<C>` unchanged at the API level (it already wraps
       BufferComponent).

  #9 — Slim `src/prelude.rs` to constructors and traits only. Move all
       configuration, placement, error, and internal-state types out of the
       prelude (e.g. `ImageBoxPlacement`, `ImageViewportPlacement`,
       `ImageViewportError`, `ScaledPixelOffset`, `UnscaledPixelOffset`,
       `ScaleBasis`, `ResizePolicy`, `StepDirection`, `ZoomDirection`,
       `ViewportAxis`, `CanvasUpdate`, etc.). These remain reachable via
       module paths (`tui_kit::image_box::Placement`).

  #10 — Reduce `ImageProtocol` from { Kitty, Sixel, ITerm2, Noop } to
        { Kitty, Noop }. Delete the dead match arms, the
        `unsupported_protocol_error` construction path, and any test
        scaffolding that referenced Sixel/iTerm2.

Hard constraints from the user:

- No backwards-compatibility shims. No deprecated re-exports. No "// kept
  for migration" comments. Rip and replace.
- Both repos can be edited atomically. If a tui-kit change breaks c4tui
  imports, fix the c4tui imports in the same task.
- Goal is end state, not graceful transition.

Repos:
  tui-kit: /Users/coleshaffer/Projects/tui-kit
  c4tui:   /Users/coleshaffer/Projects/c4tui

Key files to ground-truth before writing tasks:
  tui-kit/src/component.rs          (the three traits today)
  tui-kit/src/elements.rs           (impl Element for ... blocks; ~17 of them)
  tui-kit/src/prelude.rs            (the junk drawer)
  tui-kit/src/image.rs              (ImageProtocol + ImageBackendPreference)
  tui-kit/src/widgets/image_box.rs
  tui-kit/src/widgets/image_viewport.rs
  c4tui/src/picker.rs               (consumer of BufferComponent + Cached)
  c4tui/src/connection_picker.rs    (same)
  c4tui/src/backend.rs              (TerminalBackend trait)

Decompose into tasks following writing-plans rules (each task self-contained,
each step 2-5 minutes, TDD where the change is testable, exact file paths
and exact commands with expected output, complete code shown in every step).
Save the resulting plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-1-tui-kit-primitive-cleanup.md

Use the standard plan header from the writing-plans skill, with checkbox
syntax (`- [ ]`) for step tracking.

Cover all three items in one plan; sequence them so #3 lands first
(it's the foundation), then #9 and #10 in either order. Conclude with a
verification task that runs `cargo test` in tui-kit, then `cargo build` and
`cargo test` in c4tui, both green.
```

---

## Phase 2 — Input event boundary cleanup

### Scope

Fix the persistent naming friction at the tui-kit/c4tui boundary where `Key` carries mouse events and c4tui re-splits them into its own `InputEvent` type.

### Items covered

- **#7** — In tui-kit: split `input::Key` into `KeyEvent` (keyboard only), `MouseEvent` (click/drag/wheel/release), and `InputEvent` (the union plus `Resize`). In c4tui: delete its parallel `event::InputEvent` and use tui-kit's directly (or a thin alias if there's genuine local enrichment, which the architect should evaluate).

### Why this phase here

Doing this *before* Phase 3 means the NavPicker abstraction lands with the new event types from day one. Doing it after Phase 3 would mean re-touching every key handler in the picker code a second time.

### End state of phase

- `tui_kit::input::{KeyEvent, MouseEvent, InputEvent}` exist and are used everywhere a `Key` was used before.
- c4tui's `event::InputEvent` is gone; c4tui consumes `tui_kit::input::InputEvent` directly.
- Canvas-fraction conversion (the bit c4tui adds when re-splitting today) is either an explicit `Mouse::to_canvas_fraction(metrics)` helper on tui-kit's `MouseEvent`, or stays in c4tui as a pure function that takes `MouseEvent + CanvasMetrics` — architect's call, but no parallel enum either way.
- Every `match key { … }` in both repos has been rewritten to `match input { InputEvent::Key(...) | InputEvent::Mouse(...) | InputEvent::Resize(...) }`.
- All tests pass in both repos.

### Architect handoff prompt

```text
You're being asked to expand Phase 2 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan in the writing-plans style.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Phase 1 plan (assumed already executed):
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-1-tui-kit-primitive-cleanup.md

Phase 2 scope — one item from the architecture review:

  #7 — Split tui-kit's `input::Key` into three types:
         pub enum KeyEvent   { Char(char), Up, Down, Left, Right, Enter,
                               Esc, Tab, BackTab, Backspace, Ctrl(char), ... }
         pub enum MouseEvent { Click { x, y }, Drag { x, y },
                               WheelUp { x, y }, WheelDown { x, y },
                               Release { x, y } }
         pub enum InputEvent { Key(KeyEvent), Mouse(MouseEvent),
                               Resize { cols, rows } }
       Then in c4tui, delete the parallel `event::InputEvent` enum and use
       tui-kit's directly. The canvas-fraction conversion that c4tui does
       today when re-splitting MouseClick should either become a helper
       method on tui-kit's MouseEvent (`fn to_canvas_fraction(self, metrics: &CanvasMetrics) -> (f32, f32)`)
       or stay as a free function in c4tui — pick whichever cleaves better
       to the boundary discipline (tui-kit owns terminal-cell coordinates;
       c4tui owns canvas-fraction coordinates).

Hard constraints (still in force):

- No backwards-compatibility. No `pub use input::Key as ...` aliases. Both
  repos edited atomically.
- The exact variant set above is illustrative — keep the variants the
  current `Key` enum has, just split keyboard from mouse from
  window-resize. Don't add or remove behavior; this is a rename + split.

Files to ground-truth:
  tui-kit/src/input.rs                (the current Key enum)
  tui-kit/src/events.rs               (any tui-kit-side InputEvent)
  tui-kit/src/component.rs / elements (everything taking `Key` today)
  tui-kit/src/widgets/grid.rs         (key handling)
  c4tui/src/event.rs                  (parallel InputEvent + translate_key)
  c4tui/src/backend.rs                (translate_key on TerminalBackend)
  c4tui/src/app.rs                    (the App's main loop)
  c4tui/src/picker.rs, connection_picker.rs (current Key consumers)
  c4tui/src/keymap.rs                 (key-to-command translation)

Special attention: tui-kit's widgets/grid.rs is the most-touched key
consumer; make sure the migration covers it cleanly because NavPicker
in Phase 3 builds on Grid.

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-2-input-event-boundary.md

Standard writing-plans format: header, file structure, bite-sized TDD tasks,
exact paths, complete code in steps, frequent commits, end-of-phase
verification that runs both repos' test suites green.
```

---

## Phase 3 — NavPicker + modal-slot + image-widget winner + elements decision

### Scope

The centerpiece. Four items that the architect explicitly noted are cheaper together than separately:

### Items covered

- **#1** — Build `NavPicker<T: NavItem>` in c4tui. Port `ViewPicker` → `NavPicker<ViewNavItem>`, `ConnectionPicker` → `NavPicker<ConnectionNavItem>`, multi-child-view picker effect → `NavPicker<ChildViewNavItem>`. Three near-duplicate implementations collapse into one parameterized component.
- **#2** — Preserve the `elements` render-effect nucleus. Earlier drafts framed this as "port NavPicker through elements or delete elements." That is no longer the right test. NavPicker stays direct-ratatui/`Grid`; `elements` stays because future renderer backends need composable buffer rendering plus explicit render effects. Phase 3 should only do narrow documentation/checkpoint work here, not a broad elements refactor.
- **#4** — Pick a winner between `ImageBox` and `ImageViewport`; delete the loser. c4tui's `ViewStore::viewports` migrates to the winner. The prelude updates accordingly (this re-touches Phase 1's prelude work, which is fine).
- **#5** — Unify c4tui's three modal-slot lifecycles into one `ActiveModal` enum + one `render_modal(&mut dyn BufferComponent<Event=KeyEvent>)` method on `TerminalBackend`. The `picker_slot`, `connection_picker_slot`, `log_slot` (and the dialog) collapse. Each NavPicker variant routes its `NavOutcome::Select` through a slot-attached callback.

### Why this phase here

All four items touch the same code paths. The picker structs, the modal slots in `app.rs`, the `TerminalBackend` trait, and the image widget that the pickers and ViewStore share — those four sit on top of each other. Splitting them across phases would mean editing the same files three or four times. The architect was emphatic: these are cheaper as one atomic refactor than as a sequence.

### End state of phase

- One `NavPicker<T>` component with `NavOutcome<T> { Continue, Select(T), Cancel }`. Three call sites use it.
- One `ActiveModal` enum on `App`. One `render_modal` method on `TerminalBackend`. One `handle_modal_key` method on `App`.
- One image-viewport widget in tui-kit; the other is deleted.
- `elements` remains present as a render/effect substrate. It is not validated
  by NavPicker, and it is not expanded into a framework in this phase.
- `FakeTerminalBackend` shrinks proportionally (eleven non-core methods → ~five).
- All tests pass in both repos. New test files: NavPicker behavior, ActiveModal routing.

### Architect handoff prompt

```text
You're being asked to expand Phase 3 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan. This is the centerpiece
phase; it touches the most code of any phase in the roadmap.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Prior plans (assumed executed):
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-1-tui-kit-primitive-cleanup.md
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-phase-2-input-event-boundary.md

Phase 3 scope — four items from the architecture review, executed together:

  #1 — Build NavPicker<T: NavItem> with NavOutcome<T> { Continue, Select(T),
       Cancel }. Migrate three call sites: ViewPicker, ConnectionPicker, and
       the multi-child-view picker that the OpenChildViewPicker effect spawns.

       Suggested item-trait shape (refine as needed):
         pub trait NavItem {
             fn label(&self) -> &str;
             fn detail(&self) -> Option<&str> { None }
             fn render_row(&self, area: Rect, buf: &mut Buffer, selected: bool);
         }

       Concrete item types:
         struct ViewNavItem      { /* derived from current PickerItem */ }
         struct ConnectionNavItem { /* derived from current ConnectionPickerItem */ }
         struct ChildViewNavItem { /* derived from the inline child-view picker */ }

       The architect should design the NavPicker config struct
       (filter? search? scrolling? title?) by taking the union of features
       the three current pickers expose, dropping any that exist in only
       one and aren't worth generalizing.

  #2 — Preserve the elements render-effect nucleus.

       Updated direction: do NOT delete src/elements.rs in Phase 3, and do
       NOT use NavPicker as the validation test for elements. NavPicker is
       mostly text/grid UI and should be built directly on widgets::grid +
       ratatui primitives. The useful `elements` surface is the
       effect-carrying render tree: explicit TerminalEffect/future
       RenderEffect values, EffectElement, area-transforming containers that
       forward effects, ImageViewportElement, and grouped image teardown.

       Phase 3 should leave elements compiling, document why it is retained,
       and avoid broad reshaping. Later remote-renderer phases will decide
       whether to rename TerminalEffect to RenderEffect, move local
       application to an adapter, and shrink Window into a smaller
       EffectScope.

  #4 — Pick the winning image-viewport widget and delete the loser.
       Current state:
         tui-kit/src/widgets/image_box.rs    (907 lines, README-recommended,
                                              no production consumer)
         tui-kit/src/widgets/image_viewport.rs (1,410 lines, used by c4tui's
                                                ViewStore::viewports)

       The decision:
         - If image_box is genuinely newer + smaller + cleaner: port c4tui's
           ViewStore to it, delete image_viewport, prune prelude.
         - If image_viewport is the production-validated one: delete
           image_box, prune prelude, accept that the README was overselling
           image_box.

       Read both files and decide. Document the reasoning in the plan
       header. No third path.

  #5 — Unify three modal-slot lifecycles in c4tui/src/app.rs.

       Current shape (three near-identical handlers):
         handle_key_picker            -> picker_slot + draw_picker + close_picker
         handle_key_connection_picker -> connection_picker_slot + draw/close
         handle_key_log               -> log_slot + draw_log_view + close

       Plus a fourth: dialog_slot.

       Target shape:
         pub enum ActiveModal {
             Nav(ModalSlot<NavPicker<NavTarget>>),
             Log(ModalSlot<LogView>),
             Dialog(DialogSlot),
         }

         struct ModalSlot<C: BufferComponent<Event=KeyEvent>> {
             component: Cached<C>,
             scope_id: FocusScopeId,
             on_select: Box<dyn FnOnce(C::Message, &mut AppState) -> Effect>,
         }

       TerminalBackend trait collapses to:
         fn render_modal(&mut self, modal: &mut dyn BufferComponent<Event=KeyEvent>)
                                -> Result<()>;
         fn close_modal(&mut self) -> Result<()>;
         // ...plus the genuine non-modal methods: canvas_metrics, render,
         // teardown_image_viewport, clear_image_cache, etc.

       FakeTerminalBackend shrinks accordingly.

       NavTarget is either a single union enum
         enum NavTarget { View(ViewId), ChildView(ViewId), Connection { ... },
                          Link { ... } /* added in Phase 5 */ }
       or three separate NavPicker variants. Architect's call; both are
       defensible.

Hard constraints (still in force):

- No backwards-compatibility. Rip out the three picker structs, three
  handler methods, three slot fields, three trait-method pairs in a single
  atomic refactor. Don't preserve old names. Don't leave a deprecated path.

- This phase is allowed to be a large diff. The user has explicitly chosen
  big-step-fast over small-step-safe.

- Phase 4 (Command/Effect cleanup) is the next phase and will consume the
  NavTarget abstraction. Design accordingly — don't lock yourself out of
  the Phase 4 cleanup, but don't pre-implement Phase 4 either.

- Phase 5 (LinkDirectory) is the entire reason this work exists. NavPicker
  must accept a fourth item type (LinkCandidate) trivially in Phase 5.
  Validate this in the Phase 3 plan by sketching the LinkCandidate impl in
  a comment block at the end of the plan, even though you won't ship it.

Files to ground-truth (read all of these before writing the plan):
  c4tui/src/picker.rs                 (696 lines, ViewPicker)
  c4tui/src/connection_picker.rs      (536 lines, ConnectionPicker)
  c4tui/src/app.rs                    (modal slot duplication; ~700+ lines)
  c4tui/src/backend.rs                (TerminalBackend trait; 11 non-core methods)
  c4tui/src/state.rs                  (Effect enum; AppState; the OpenChildViewPicker variant)
  c4tui/src/view.rs                   (ViewStore::viewports; will be split in Phase 6, leave alone here except for the image-widget migration in #4)
  tui-kit/src/widgets/grid.rs         (NavPicker substrate)
  tui-kit/src/widgets/image_box.rs    (#4 winner candidate)
  tui-kit/src/widgets/image_viewport.rs (#4 winner candidate)
  tui-kit/src/elements.rs             (#2 preservation checkpoint target)
  tui-kit/src/component.rs            (unified trait from Phase 1)
  tui-kit/src/prelude.rs              (re-touched here for #4)

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-3-navpicker-modal-image-elements.md

Plan structure expectations:
- Header section that BAKES IN the #2 decision (preserve the render-effect
  nucleus; do not port NavPicker through elements) and the #4 decision
  (image_box vs image_viewport winner), with one paragraph of justification
  for each. These decisions are not subtasks; they're plan preconditions.
- File-structure section listing every file created or modified.
- Tasks ordered as: (i) introduce NavPicker shape (TDD-able in isolation
  before any porting); (ii) migrate first picker, run tests; (iii) migrate
  second picker; (iv) introduce ActiveModal; (v) collapse trait methods;
  (vi) image-widget swap; (vii) elements preservation checkpoint; (viii)
  full verification.
- Each task self-contained with complete code, exact paths, TDD where
  applicable, frequent commits.

This is the largest phase. Don't shy from a long plan — but every step
must still be bite-sized and concrete.
```

---

## Phase 4 — Command + Effect cleanup

### Scope

c4tui-only cleanup of two parallel enums and an effect channel that's mixing concerns.

### Items covered

- **#8** — Collapse `PendingCommand` and `Command` into one enum. The split exists today because three variants need canvas metrics; the cost applies to all ~30. Use `Option`-typed canvas fields filled in by the app, or have the keymap return `Result<Command, NeedsCanvas>` with the second variant carrying only the three canvas-needing shapes.
- **#11** — Move cycling effects (`CycleScaleBasis`, `CycleOverflow`, `CycleZoomStep`) out of `Effect` into direct state mutations in `AppState::apply`. After Phase 3's modal-slot work, the `OpenPicker` / `OpenChildViewPicker` / `OpenConnectionPicker` variants also collapse into `OpenModal(ModalSpec)` — capture that here.

### Why this phase here

Phase 3 reshapes the modal-side of `Effect`. Doing #8 and #11 immediately after, while the Effect/Command code is fresh, is cheaper than coming back to it later. Both are small.

### End state of phase

- One `Command` enum in c4tui (no `PendingCommand`).
- `Effect` is narrower: navigation + workspace lifecycle + quit + `OpenModal`. Cycling variants are gone (moved into `AppState::apply`).
- Keymap returns `Command` directly (or `Result<Command, NeedsCanvas>` with a 3-variant secondary type).
- Tests pass.

### Architect handoff prompt

```text
You're being asked to expand Phase 4 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Prior plans (assumed executed):
  Phase 1-3 plans in /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/

Phase 4 scope — two items from the architecture review:

  #8 — Collapse PendingCommand and Command into one enum in c4tui/src/event.rs.

       Current shape: PendingCommand has ~30 variants; Command has ~30
       variants; PendingCommand::resolve(canvas) -> Command is a 40-line
       one-to-one mapping. Only three variants actually differ:
         PendingCommand::Inspect          -> Command::InspectAt { x, y }
         PendingCommand::Zoom { factor }  -> Command::Zoom { factor, anchor: Center }
         PendingCommand::ZoomAt { ... }   -> Command::Zoom { factor, anchor: Canvas { ... } }
         PendingCommand::DragTo { x, y }  -> Command::DragTo { x, y, canvas }
       (Four if you count DragTo.)

       Target shape — architect's call between two options, document choice
       in plan header:
         (A) One Command enum. Variants that need canvas metrics carry
             Option<(f32, f32)> or similar; the app fills the Option after
             receiving the command. Keymap returns Command directly.
         (B) Two enums but explicit about scope: a small NeedsCanvas enum
             with only the three (or four) canvas-needing variants;
             everything else is just Command. Keymap returns
             Result<Command, NeedsCanvas>.

       Prefer (A) for minimal surface; (B) if leaving the canvas dependency
       in the type signature is genuinely more readable.

  #11 — Reshape Effect in c4tui/src/state.rs:

        Current 11 variants split into three categories:
          - Modal/navigation: OpenPicker, OpenChildViewPicker,
                              OpenConnectionPicker, ShowHelp, ToggleLogView
          - Lifecycle: ReloadWorkspace, ClearImageCache, Quit
          - Cycling (these don't belong here): CycleScaleBasis,
                                               CycleOverflow, CycleZoomStep

        After Phase 3, the first three modal variants should already have
        been collapsed into OpenModal(ModalSpec). If Phase 3 didn't ship
        that collapse, do it here.

        Move the cycling variants OUT of Effect and INTO direct state
        mutations in AppState::apply. The keymap was issuing these as
        effects only because Effect was the universal "the user did
        something" channel; that's the wrong reason. They're pure state.

        Resulting Effect should be roughly:
          enum Effect {
              OpenModal(ModalSpec),
              ShowHelp,
              ToggleLogView,
              ReloadWorkspace,
              ClearImageCache,
              Quit,
          }
        (Trim further if some of these are also pure state mutations on
        AppState — architect's judgment.)

Hard constraints (still in force):

- No backwards-compatibility. Delete PendingCommand outright. Delete the
  cycling Effect variants outright.
- Rip out resolve(). Don't keep it as a no-op or alias.

Files to ground-truth:
  c4tui/src/event.rs                  (PendingCommand, Command, resolve)
  c4tui/src/state.rs                  (Effect, AppState, AppState::apply)
  c4tui/src/keymap.rs                 (the keymap that produces commands)
  c4tui/src/app.rs                    (where Effect is consumed; post-Phase-3 shape)

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-4-command-effect-cleanup.md

Standard writing-plans format. Small phase; expect 6-10 tasks total.
End with verification: cargo test in c4tui passes.
```

---

## Phase 5 — LinkDirectory implementation

### Scope

The reason the whole roadmap exists. Implement keyboard-first navigation through a "mini link directory" of immediately reachable diagrams as the canonical NavPicker configuration.

### Items covered

- **#6** — Implement `LinkDirectory` as `NavPicker<LinkCandidate>` (or as a configuration thereof). `LinkCandidate` carries: target view, source element (the thing the link is "from"), label, and the post-navigation focus hint (`pinned_element_after_navigation`).

### Why this phase here

NavPicker (Phase 3) and the modal-slot unification (Phase 3) and the Effect cleanup (Phase 4) are all prerequisites. With those in place, LinkDirectory lands as a configuration of an existing component plus a new `NavTarget` variant — *not* a new picker, not a new modal type, not a new trait method.

### End state of phase

- LinkDirectory is reachable via a keystroke (the architect should pick which — likely `l` or `Enter` on a focused element).
- LinkCandidate enumeration: given the current view and focused element, the directory lists every diagram immediately reachable from this position. The c4tui architecture doc's §6.2 describes the candidate-computation logic; the architect should expand it into concrete code.
- Selecting a candidate navigates and (optionally) pins focus on a post-navigation element.
- Mouse drill (click-to-drill) is preserved but no longer the primary path; the spec/README revisions from the recent doc pass already reflect this.
- Tests pass; the c4tui spec's keyboard-first behavior is exercised by at least a happy-path integration test.

### Architect handoff prompt

```text
You're being asked to expand Phase 5 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan. This is the goal phase
— the entire roadmap is in service of it.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Prior plans (assumed executed):
  Phases 1-4 in /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/

Phase 5 scope — one item from the architecture review:

  #6 — Implement LinkDirectory. Concretely:

       (a) Define LinkCandidate in c4tui:
             struct LinkCandidate {
                 target_view: ViewId,
                 source_element: Option<ElementId>,
                 label: String,
                 pinned_element_after_navigation: Option<ElementId>,
             }
           and make it impl NavItem (from Phase 3).

       (b) Implement candidate computation. The c4tui spec/architecture
           describe the directory as "every diagram immediately reachable
           from the current view + focused element." Concretely this means:
             - For each Relationship in the current view's c4 model whose
               source or destination is the focused element (or any element
               if no focus), if the other end belongs to a different view,
               that's a candidate.
             - The "linked from here" semantics from c4tui/architecture.md
               §6.2 dictate the exact filter; the architect should read
               that section and implement it faithfully.
           This logic likely belongs in a new file: c4tui/src/link_directory.rs.

       (c) Wire LinkDirectory into the modal infrastructure (from Phase 3).
           It becomes a NavTarget variant (or its own modal variant — choose
           whichever the Phase 3 plan committed to). A keystroke on the
           current view opens it; selecting a candidate emits a Navigate
           command that switches view and optionally pins focus.

       (d) Update c4tui/specification.md and c4tui/architecture.md to
           reflect the implemented model (the docs currently SKETCH it
           rather than describe it). The README's "keyboard-first" framing
           is already in place from the recent doc pass; revise as needed.

Hard constraints:

- No backwards-compatibility. The existing pickers (ViewPicker,
  ConnectionPicker, multi-child-view picker, plus LinkDirectory) ALL use
  the same NavPicker abstraction by now — verify this in the plan header
  by listing each call site and confirming it's a NavPicker<T> variant.
- Mouse drill remains functional but the spec language must frame it as
  secondary, per the recent doc pass.
- The keystroke that opens LinkDirectory should be documented and bound
  in the keymap. Architect picks the binding; suggested defaults: 'l'
  (link) or '/' (search), or Tab if it composes with existing focus
  navigation cleanly.

Files to ground-truth before writing the plan:
  c4tui/specification.md              (the spec for LinkDirectory)
  c4tui/architecture.md, especially §6.2 (LinkCandidate sketch)
  c4tui/README.md                     (keyboard-first framing)
  c4tui/src/state.rs                  (AppState; c4 model accessor)
  c4tui/src/view.rs                   (post-Phase-6 may be split; here, just read it)
  c4tui/src/keymap.rs                 (where new keystroke binds)
  c4tui/src/app.rs                    (post-Phase-3 ActiveModal)
  ANY existing picker code (post-Phase-3 NavPicker)

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-5-link-directory.md

Standard writing-plans format. Include:
  - A task for candidate-computation logic with TDD (this is pure and
    very testable).
  - A task for the integration test that opens LinkDirectory, selects a
    candidate, and verifies the resulting view + focus state.
  - Doc-update tasks for specification.md and architecture.md.

End-state verification: a manual-test checklist for keyboard-first
navigation through a small workspace, plus passing cargo test.
```

---

## Phase 6 — ViewStore split

### Scope

c4tui-only refactor. Split the 886-line `ViewStore` into three concerns sharing a `ViewId`.

### Items covered

- **#12** — Split `ViewStore` into:
  - `ViewCatalog` — view metadata, workspace model, export-directory guard.
  - `RenderCache` — `HashMap<ViewId, RenderedView>`, raster budget.
  - `ViewportState` — per-view image-widget instances (whichever widget won Phase 3 #4), transform state, placement policy.

### Why this phase here

Phase 3 makes pickers consume `&ViewCatalog` instead of `&ViewStore` (because they only need catalog data), so the split becomes natural rather than forced. Doing it before Phase 3 would require re-touching the pickers; doing it after Phase 5 is also fine but the split clarifies what LinkCandidate computation actually depends on.

The architect noted this is the right ordering: the pickers stop depending on ViewStore in Phase 3, which makes the split obvious. Phase 5 (LinkDirectory) only needs `&ViewCatalog` for candidate computation, so doing #12 between Phase 5 and Phase 7 is correct.

### End state of phase

- `ViewStore` no longer exists. Three new types: `ViewCatalog`, `RenderCache`, `ViewportState`. `App` holds all three.
- Every call site that took `&ViewStore` now takes the narrowest of the three it actually needs.
- Tests pass.

### Architect handoff prompt

```text
You're being asked to expand Phase 6 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Prior plans (assumed executed): Phases 1-5.

Phase 6 scope — one item from the architecture review:

  #12 — Split c4tui/src/view.rs's ViewStore (886 lines, three responsibilities)
        into three structs:

          pub struct ViewCatalog {
              pub views: Vec<ViewMeta>,
              pub model: c4::Workspace,        // or whatever the type is
              pub export_guard: ExportDirGuard,
          }

          pub struct RenderCache {
              pub rendered: HashMap<ViewId, RenderedView>,
              pub budget: RasterBudget,
          }

          pub struct ViewportState {
              pub viewports: HashMap<ViewId, ImageBox /* or ImageViewport, whichever won Phase 3 */>,
              pub policy: PlacementPolicy,
          }

        App holds all three. Call sites narrow from `&ViewStore` to
        whichever single struct they actually need. The architect should
        audit every current consumer of ViewStore and reassign each to
        ViewCatalog / RenderCache / ViewportState.

        File layout: probably three files
          c4tui/src/view_catalog.rs
          c4tui/src/render_cache.rs
          c4tui/src/viewport_state.rs
        with view.rs becoming either empty (delete) or just re-exports
        (also delete — no backwards-compat).

Hard constraints:

- Rip out ViewStore. No alias.
- The export-directory guard is a Drop-bearing type; make sure its lifetime
  still terminates exactly when the app exits (today: when ViewStore drops).
  ViewCatalog owns it now.
- Every call site updated. The compiler will help find them once ViewStore
  is gone.

Files to ground-truth:
  c4tui/src/view.rs                   (the current ViewStore)
  c4tui/src/render.rs                 (the renderer)
  c4tui/src/picker.rs / connection_picker.rs / link_directory.rs (consumers)
  c4tui/src/app.rs                    (App's fields)
  c4tui/src/state.rs                  (AppState, the c4 model accessor)
  c4tui/src/event.rs                  (Command handlers that touch viewports)

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-6-viewstore-split.md

Standard writing-plans format. End-state verification: cargo test passes,
plus a manual-test of "load workspace, navigate, reload" to confirm the
export-directory lifetime is intact.
```

---

## Phase 7 — Documentation + retention tooling + render-effect docs

### Scope

Doc-heavy and tooling-light phase that locks in everything the prior phases
produced, and explicitly records the render/effect/backend split that will feed
the later remote-renderer phases.

### Items covered

- **#13** — Promote the "real consumer" rule from an addition test to a retention test. Add a CI audit (a small Rust script or `cargo` plugin invocation, or a shell script) that lists tui-kit's public types and flags any with zero references in c4tui or in tui-kit's own tests.
- **#14** — Reconcile README + specification.md + architecture.md in both repos against the actual post-refactor code state. After Phase 3 in particular, the docs need to match reality: which trait, which image widget, and the fact that `elements` remains as a render-effect substrate rather than a NavPicker substrate.
- **New direction** — Add a concise render-backend note to tui-kit docs: local terminal rendering is implemented today; remote-renderer protocol and SSH client tooling are future work; render effects are the bridge.
- **#15** — Archive or restructure `c4tui/implementation-plan.md`. The current document describes Phases 0-6 of c4tui's *original* implementation, which is finished. Either rename to `docs/historical-plan-phase-0-6.md` or add a Status: COMPLETE header plus a forward-looking "Phase 7" section that points at this roadmap.
- **#16** — Add the SVG→PNG→Kitty→placement image-pipeline stage diagram in `c4tui/architecture.md`. The architect's review noted this absence is why ViewStore's three jobs were conflated; the diagram makes the Phase 6 split self-evident.
- **#17** — Write a short cross-repo testing-patterns doc. After Phase 3 the `FakeTerminalBackend` is much smaller and the gap between it and tui-kit's `testkit` has narrowed; document the patterns side-by-side so they don't drift apart again.

### Why this phase last

All the doc items can only be written truthfully after the code lands. Doing them piecemeal during earlier phases would mean rewriting them every phase. Doing them all together at the end is cheaper.

The retention-tooling item could in principle go earlier, but it's most useful after the deletions (#2, #4, #10) have already happened — it then prevents *new* speculative surfaces from accumulating.

### End state of phase

- A CI script that fails if a tui-kit public type has zero in-tree references.
- README, specification.md, architecture.md in both repos accurate and agreeing with each other and with the code.
- tui-kit docs describe `elements` as composable buffer rendering plus explicit
  render effects, not as a retained UI framework.
- `c4tui/implementation-plan.md` clearly historical, not roadmap.
- Image-pipeline stage diagram in `c4tui/architecture.md`.
- A `docs/testing-patterns.md` (or similar) in one of the repos describing the testkit / FakeTerminalBackend pattern pair.

### Architect handoff prompt

```text
You're being asked to expand Phase 7 of the tui-kit + c4tui refactor roadmap
into a concrete, task-by-task implementation plan. This is mostly a
documentation phase plus one small tooling item.

Roadmap document: /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md
Prior plans (assumed executed): Phases 1-6.

Phase 7 scope — five items from the architecture review:

  #13 — Build a CI audit script that promotes the "named consumer per
        change" rule into a "retention test." Concrete shape: a small
        script (Rust, shell, or whatever fits) that:
          (a) enumerates tui-kit's public types (rustdoc JSON output, or
              a grep-based heuristic over `pub` items),
          (b) for each, greps c4tui/src and tui-kit/tests for at least one
              non-self reference,
          (c) reports unreferenced types and exits non-zero in CI.
        Add it to whatever CI runner the repos use (the architect should
        check tui-kit/.github/workflows and c4tui/.github/workflows; if
        there's no CI yet, document the script as a manual command to run
        before merging).

  #14 — Reconcile README.md, specification.md, architecture.md in BOTH
        repos against the actual post-Phase-6 code. Specifically:
          - tui-kit/specification.md sections 4.x (public surfaces) should
            list exactly what survived after Phases 1, 3, 6. `elements`
            should be documented as the render/effect composition layer,
            not as a retained app runtime. If image_box was deleted (or
            image_viewport), the corresponding section goes.
          - tui-kit/architecture.md should describe the unified trait,
            single image widget, slimmed prelude, and local/future-remote
            renderer boundary.
          - tui-kit/README.md should match.
          - c4tui/specification.md should describe LinkDirectory as
            implemented, not as sketched.
          - c4tui/architecture.md should describe ViewCatalog/RenderCache/
            ViewportState split, NavPicker, ActiveModal.
          - c4tui/README.md should match.

  #15 — Either archive c4tui/implementation-plan.md (rename to
        c4tui/docs/historical-plan-phase-0-6.md) and replace with a
        pointer to this roadmap, OR add a "Status: COMPLETE" header to
        the existing plan plus a "Phase 7" section that defers to this
        roadmap. Architect's call.

  #16 — Add an image-pipeline stage diagram to c4tui/architecture.md
        showing the three stages explicitly:
          Stage 1: SVG -> PNG     (async, RenderPool)
          Stage 2: PNG -> Kitty image registry (synchronous, first display)
          Stage 3: Image -> placement (every interaction, ImageBox or
                                       ImageViewport — whichever survived)
        with the owning type at each arrow. Mermaid or ASCII art is fine.

  #17 — Write docs/testing-patterns.md (in tui-kit, with a pointer from
        c4tui, OR vice versa — architect's call) describing the two test
        patterns:
          - tui-kit: testkit (mock surfaces, deterministic scheduler,
                              buffer assertions)
          - c4tui: FakeTerminalBackend (post-Phase-3 simplified shape)
        with one worked example per pattern, and guidance on when to
        reach for which.

Hard constraints:

- The CI script (#13) should be straightforward and small. Don't overbuild
  it. If rustdoc JSON is hard to parse, a `grep -r 'pub ' tui-kit/src/ |
  ...` + `grep -r 'tui_kit::' c4tui/src/ tui-kit/tests/` heuristic is fine
  for a first version.
- Doc items (#14, #15, #16, #17) must reflect the actual post-Phase-6 code.
  Don't write aspirational docs.

Files to ground-truth:
  All README.md, specification.md, architecture.md files (both repos).
  c4tui/implementation-plan.md.
  Whatever CI configuration exists.

Save plan to:
  /Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/
    2026-05-12-phase-7-docs-and-retention.md

Standard writing-plans format. Many of these tasks won't have unit-test
verification (they're docs); for those, the verification step is
"re-read the doc top-to-bottom and confirm it matches the code." For the
CI script (#13), include a test that runs it against the current repo and
verifies it produces the expected output (empty after Phases 1-6, since
deletions happened).
```

---

## Phase 8 — RenderEffect + renderer backend model

### Scope

tui-kit-only design and implementation phase for the structured render/effect
model needed before any SSH client exists.

### Items covered

- Rename or split `TerminalEffect` into a renderer-neutral `RenderEffect`
  concept if the code shape warrants it.
- Keep local terminal application as an adapter: render effects apply to
  `ImageSurfaceRegistry` today, but the effect values themselves should be
  serializable in principle.
- Define a minimal renderer backend boundary that can consume a buffer plus
  render effects.
- Add an in-process mock renderer that proves the boundary without network,
  SSH, or subprocess complexity.
- Shrink `elements` only where it directly supports that boundary. Containers
  that transform child areas and forward effects stay; retained-runtime or
  product-policy behavior should be removed or isolated.

### Why this phase here

The remote idea needs a clean local protocol before it needs SSH. This phase
turns the current `elements` nucleus into a deliberate renderer contract while
keeping the implementation testable in one process.

### End state of phase

- A small, documented render/effect contract in tui-kit.
- Local terminal rendering still works through the existing terminal/image
  stack.
- A mock renderer can accept a rendered buffer and render effects and assert
  the same placement/teardown behavior as the local adapter.
- No SSH, persistent agent, host registry, or client profile format yet.

---

## Phase 9 — `tui-kit-cli` remote launcher prototype

### Scope

Prototype the user-facing client path after the render/effect model exists.

### Items covered

- Add a `tui-kit-cli` binary or workspace crate that can launch a remote app
  through SSH with a PTY/stdin-stdout transport.
- Define a small handshake so the remote app can detect the client renderer and
  fall back to ordinary terminal mode when absent.
- Forward typed input, mouse, and resize events from the local client to the
  remote app.
- Render remote buffer/effect frames locally using the Phase 8 renderer
  backend.
- Add a small profile/config format for hostnames and app commands only after
  the basic `run ssh://host -- app` path works.

### Why this phase here

The client helper is a product surface and a security boundary. It should not
exist until tui-kit has a stable enough local renderer contract to avoid
streaming arbitrary terminal bytes or relying on local screen capture.

### End state of phase

- `tui-kit-cli run ssh://host -- app` can launch a compatible remote tui-kit
  app and render it locally.
- Plain `ssh -t host app` remains a fallback path for apps that choose normal
  terminal mode.
- The client exposes only allowlisted rendering/input capabilities. No
  arbitrary local command execution and no implicit window/screen capture.

---

## Appendix — Source review priority table

For reference, here's the architect's prioritized list from the 2026-05-12 review. The numbers in this roadmap (#1 through #17) refer to these:

| # | Refactor | Repo | Phase |
|---|----------|------|-------|
| 1 | Unify ViewPicker + ConnectionPicker + future LinkDirectory + multi-child-view picker into `NavPicker<T>` | c4tui | Phase 3 |
| 2 | Preserve and later reshape `elements` as render/effect substrate | tui-kit | Phase 3, Phase 8 |
| 3 | Collapse `Component` / `BufferComponent` / `Element` into one trait | tui-kit | Phase 1 |
| 4 | Pick a winner between `ImageBox` and `ImageViewport`; delete the loser | tui-kit | Phase 3 |
| 5 | Unify c4tui's three modal-slot lifecycles into one `ActiveModal` + one `render_modal` | c4tui | Phase 3 |
| 6 | Sequence LinkDirectory behind #1 and #5 (implementation phase) | c4tui | Phase 5 |
| 7 | Split `Key` into `KeyEvent` + `MouseEvent` + `InputEvent`; collapse c4tui's parallel enum | both | Phase 2 |
| 8 | Collapse `Command` and `PendingCommand` into one enum | c4tui | Phase 4 |
| 9 | Slim tui-kit prelude to constructors + traits | tui-kit | Phase 1 |
| 10 | Reduce `ImageProtocol` to `{ Kitty, Noop }` | tui-kit | Phase 1 |
| 11 | Split cycling effects out of `Effect` into direct state mutations | c4tui | Phase 4 |
| 12 | Split `ViewStore` into catalog / render-cache / viewport-state | c4tui | Phase 6 |
| 13 | Promote "real consumer" rule to a retention test (CI audit) | both | Phase 7 |
| 14 | Reconcile README + spec + architecture after the above land | both | Phase 7 |
| 15 | Archive c4tui's `implementation-plan.md` | c4tui | Phase 7 |
| 16 | Add stage-1/2/3 image-pipeline diagram in c4tui architecture | c4tui | Phase 7 |
| 17 | Write short cross-repo testing-patterns doc | both | Phase 7 |

All 17 review items are covered across the first seven phases. Phases 8-9 are
new directional work, not part of the original review list.

---

## Execution notes

- **Inter-phase commits:** Each phase should end on green tests in both repos and a clean commit (or commit series). Do not start the next phase's architect prompt until the prior phase's plan is fully executed and committed.
- **Plan vs roadmap:** This document is the roadmap. The per-phase plans the architect will produce are the actionable artifacts. Treat this document as durable; treat per-phase plans as one-shot.
- **Order is significant.** Phase 1 unblocks Phase 3's trait usage. Phase 2 unblocks Phase 3's event types. Phase 3 unblocks Phases 4-6. Phase 5 is the c4tui goal. Phases 8-9 must not start until the render/effect contract is documented and the local app cleanup is complete. Don't reorder without re-running the dependency check.
- **If a phase plan reveals the order is wrong:** stop, update this roadmap, then re-run the affected phase prompts. Don't silently improvise.
