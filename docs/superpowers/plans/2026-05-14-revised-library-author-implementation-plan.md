# Revised Library-Author Implementation Plan

**Status:** ACTIVE · 2026-05-14
**Role:** Operational source of truth for the library-author path. Supersedes
[`2026-05-13-fresh-library-author-plan.md`](./2026-05-13-fresh-library-author-plan.md)
as the doc execution agents read first.
**Inherits:** [`2026-05-13-library-author-direction.md`](./2026-05-13-library-author-direction.md)
(thesis, non-goals, operator boundary). Direction principles are non-negotiable.
**Phase A status:** shipped (`7d47d59`, `2e33ae2`, `049e418`).

## Why this doc exists (and why it supersedes rather than sits alongside)

The fresh plan is correct at the phase level. Two things still hurt execution:

1. Each phase carries a checklist but no Dependencies / Non-goals / Risks
   triple, so a fresh executor has to re-derive prerequisites and re-justify
   ordering. That work belongs in the plan, not in the head of the next agent.
2. The phase order in the fresh plan (`A → B → C → D → E → F`) reads as a
   sequence even though the doc already flags that Phase C is gated on Phase F.
   The dependency graph is not linear; pretending it is invites speculative
   deletion in Phase C and stalled work in Phases D/E.

Supersede (rather than amend in place) because the changes are structural — a
new dependency graph, expanded per-phase contracts, and a sharpened scope
recommendation for `src/elements.rs` — and because the fresh plan is itself
durable history of the direction change. Leave it in place as the strategic
record; this doc is what executors open. **Confidence: HIGH.**

## Goal

Identical to the fresh plan: tui-kit becomes an excellent Rust TUI library
that works locally, survives SSH/container environments where the terminal
permits, and preserves a transport-safe render/effect contract without
committing to a transport protocol. c4tui validates everything.

## Architecture

The architectural surface is captured in:

- `architecture.md` — overall module map and §8 "Render Effects and Image
  Lifecycle" (the load-bearing section for Phase B);
- `specification.md` — public-contract description, particularly §5
  ("Behavioral Contracts") on serializable render intent;
- `src/lib.rs` — the published module list (no `elements` deletion in scope);
- `src/prelude.rs` (lines 21-23) — currently re-exports `EffectElement`,
  `Element`, `ElementExt`, `ElementOutcome`, `ImageViewportElement`. The Phase B
  rename touches the first of these; later phases may add/remove others.

Key architectural fact discovered while reviewing: **`src/elements.rs` is
structurally isolated.** No other module in `src/` (including `widgets/`,
`testkit.rs`, and `tests/parity.rs`) references any item exported by
`elements`. The only external consumer is the prelude, and the only external
*user* of the prelude is c4tui — which consumes none of the items reachable
through `elements` (zero hits for `Element`, `EffectElement`, `Bordered`,
`Padded`, `Panel`, `Stack`, `Window`, `Modal`, `Overlay`, `Text`, `Focusable`,
`KeyMapped`, `ScrollY`, `ImageViewportElement`, `TerminalEffect`, or
`RenderEffect` across `/Users/coleshaffer/Projects/c4tui/src/`). c4tui uses
`BufferComponent`, `Cached`, and the `widgets::grid::Grid` primitives directly.

That isolation makes Phase B safe to execute as a contained, single-file
refactor (verified against `/Users/coleshaffer/Projects/tui-kit/src/elements.rs`
lines 1-2625 of production code; tests live at lines 2626-3727). It also
sharpens Phase C: the retained-widget machinery in `elements.rs` currently has
**no proven external consumer at all**. The non-goal "do not delete
speculatively" still holds because Phase F may pull it back in, but the
empirical baseline is "zero consumers today," not "unclear consumers."

## Tech stack

Unchanged from the fresh plan: ratatui 0.29, crossterm 0.28, `anyhow`,
`serde`, Kitty graphics protocol via `src/image.rs`. No new runtime or
transport dependencies are introduced by any phase below.

## Non-negotiable constraints (inherited)

- No transport protocol design.
- No frame format or wire schema.
- No `tui-kit-agent`.
- `RenderEffect` is data-only by the concrete criteria in Phase B.
- c4tui is treated as a path-dep contract; do not break it.
- No graceful-migration / deprecation shims — rip-and-replace at source.
- Live terminal / SSH / container smoke tests are operator-only.

## Dependency graph (the corrected one)

The fresh plan reads `A → B → C → D → E → F`. The true graph is:

```text
A (shipped)
   │
   ├── B  (RenderEffect contract: extract + rename + data-only enforcement)
   │      │
   │      ├── D  (image path + SSH/container reliability, operator-gated)
   │      │
   │      └── E  (testkit hardening around render effects)
   │
   └── F  (c4tui validation work; concurrent with B once B's rename lands,
          but does not depend on B beyond not breaking the prelude)
          │
          └── C  (elements triage; gated on F outcomes — only c4tui's
                 convergence reveals which retained widgets have a consumer)
```

Key edges:

- **B → D, B → E.** D and E both want to assert behavior in terms of the
  renamed, data-only effect type. Starting them before B leaves rework on the
  table.
- **F is independent of B's rename.** c4tui does not consume `TerminalEffect`,
  so the rename does not stall c4tui work. Treat F as runnable concurrently.
  **Confidence: HIGH.**
- **F → C, not B → C.** Phase C decides whether `Panel`/`Stack`/`Window`/
  `Modal`/`Overlay`/`WindowChrome`/`KeyScope`/`ImageViewportElement` survive.
  None of those types are consumed by c4tui today. Phase F may pull some of
  them in (e.g. NavPicker consolidation could discover `Stack` is useful);
  until F's sub-decisions land, C cannot honestly delete. **Confidence: HIGH.**
- **C is also gated by B's extract step.** The Phase B extract sub-step
  (carving `RenderEffect` + `EffectElement` out of `elements.rs`) is what makes
  C's "shrink and delete decisively" mechanically cheap — the surviving
  retained types stop being entangled with the load-bearing effect contract.
  **Confidence: HIGH.**
- **D and E can interleave.** E gives D testable assertions for image
  lifecycle; D gives E real edge cases. Either can lead. Operator-blocking
  steps in D do not block E's library-side work.

The fresh plan's linear ordering was a presentation choice. This plan treats
B as the next on-path work, F as the parallel consumer-driven track, D and E
as fan-out from B, and C as the final reconciliation gated on F.

## Plan-level Exit criteria (sharpened from "Plan Complete When")

The plan as a whole is done when every item below is true. Each item is
either a code/test invariant agents can verify or an artifact the operator
signs off on.

1. `RenderEffect` is the only effect-enum name in `src/` (and in c4tui if it
   ever takes a direct dependency on the type). The enum derives
   `Clone + Debug + PartialEq + Eq`; contains no `Box<dyn _>`, no `Fn`/closure
   fields, no `Arc<Mutex<_>>` live-state handles, and no method that requires
   ambient access to terminal globals or app state. Verified by `cargo check`
   plus the existing apply-to-surface / apply-to-registry tests
   (`src/elements.rs` lines 89-110 today; will move with the extraction).
2. `EffectElement` and `RenderEffect` live in a dedicated module
   (`src/elements/effect.rs` per Phase B), with module-level rustdoc that
   states the data-only contract explicitly and points at
   `architecture.md §8`.
3. `architecture.md §8` and `specification.md §4.4`/§5 use "render effect" /
   `RenderEffect` consistently. No "terminal effect" / `TerminalEffect`
   language remains in active docs or active plans.
4. `src/elements.rs` has either been split or decisively trimmed: it is
   either ≤1,200 production lines, or every retained widget in it has a named
   consumer (in c4tui, in `widgets/`, or in a test that encodes a real
   render/effect invariant). The retained set is documented in
   `architecture.md`.
5. c4tui no longer carries placeholders for unresolved tui-kit boundaries:
   `NavPicker` consolidation has landed, modal handling is unified around the
   smallest useful abstraction (the `Modal` trait sketched in
   `docs/superpowers/plans/2026-05-12-phase-3-navpicker-modal-image-elements.md`
   or its successor), the single surviving image-viewport widget is wired
   through c4tui, and `LinkDirectory` has shipped.
6. The operator has signed off on local + SSH + SSH-into-container +
   `docker exec` smoke tests for the image path, with results recorded under
   `docs/superpowers/handoffs/`.
7. `testkit` can verify render output and render effects without a live
   terminal emulator, including helper assertions for the `RenderEffect`
   shape. The docs (likely an addition to `architecture.md §10`) say when to
   use tui-kit `testkit` versus c4tui's fake backend.
8. The widget table in `architecture.md §2` reflects the surviving modules
   only. The prelude (`src/prelude.rs`) re-exports only items with a named
   consumer or a test encoding a library-level invariant.

When all eight hold, tui-kit is library-author-ready. Further protocol or
renderer-backend work becomes pull-driven from "Deferred Pulls" in the fresh
plan, not committed roadmap.

---

## Phase A — Baseline and Direction Cleanup (SHIPPED)

**Status:** Shipped. Tracked here for the dependency graph only.

Shipped in commits `7d47d59` (direction doc + fresh plan + supersede stamp),
`2e33ae2` (architecture/specification/README alignment), and `049e418`
(historical phase plans and handoffs tracked). Future executors do not run
Phase A; reference it only when a downstream phase needs to point at a
doc-alignment fact already in `architecture.md` or `specification.md`.

## Phase B — RenderEffect Contract (NEXT)

**Purpose:** Promote the effect contract as tui-kit's load-bearing idea, give
it its own module, enforce the data-only criteria, and rename the type so
docs and code use the same renderer-neutral vocabulary.

### Pre-conditions and verified state

- `TerminalEffect` and `EffectElement` are referenced only in
  `/Users/coleshaffer/Projects/tui-kit/src/elements.rs`. Production code spans
  lines 1-2625; tests span lines 2626-3727. The enum is defined at lines
  53-72; the apply methods are at lines 74-111; the `EffectElement` trait is
  at lines 113-120. Concrete impl sites: `Panel` (601), `Stack` (835),
  `Focusable` (1133), `KeyMapped` (1243), `Padded` (1329), `Bordered` (1411),
  `Window` (1994), `ImageViewportElement` (2270), `Modal` (2412),
  `Overlay` (2608). **Confidence: HIGH** (grep verified).
- `c4tui/src/` has zero references to `TerminalEffect`, `EffectElement`, or
  `RenderEffect`. **Confidence: HIGH** (grep verified).
- The current enum already derives `Clone, Debug, PartialEq, Eq` and is
  `#[non_exhaustive]` (lines 51-53). It contains `Arc<[u8]>` for PNG bytes,
  which is fine (it is owned bytes, not a live-state handle). It does **not**
  contain `Box<dyn _>`, closures, or ambient handles today. The data-only
  criteria are already met; Phase B's job is to lock that in with explicit
  rustdoc and a structural test, not to fix violations. **Confidence: HIGH.**

### Dependencies

- Phase A doc alignment (shipped).
- No upstream dependency on c4tui; Phase F runs in parallel.

### Scope (phase-level; bite-sized plan written at execution time)

1. **Extract first, rename second.** Carve `RenderEffect` (currently
   `TerminalEffect`), `EffectElement`, and the apply methods into
   `src/elements/effect.rs`. Convert `src/elements.rs` into
   `src/elements/mod.rs` to preserve the import surface, or, if the broader
   rationalization in the "elements.rs scope check" section below proceeds,
   directly into `src/elements/` as a directory with `mod.rs`, `effect.rs`,
   and the other recommended modules.
2. **Rename `TerminalEffect → RenderEffect`** and `terminal_effects(...) →
   render_effects(...)` (and `teardown_effects` stays — the word "teardown"
   is renderer-neutral). Update the 27 impl sites and the 93 mention sites
   in one pass; there is no path-dep consumer to coordinate with.
3. **Lock the data-only contract.** Add module-level rustdoc to
   `src/elements/effect.rs` that lists the five constraints (derives
   `Clone + Debug + PartialEq + Eq`; no `Box<dyn _>`; no `Fn`/closure fields;
   no `Arc<Mutex<_>>` live state; no ambient terminal/app-state access). Add
   a compile-only test that constructs each variant and round-trips it
   through `Clone + Debug + PartialEq` to make accidental violation noisy.
   ("Serializable in principle" follows from these; do not commit to a
   serde representation in Phase B.)
4. **Update `architecture.md §8`** so the section title and body refer to
   `RenderEffect`, and so it explicitly states the data-only contract. Cross-
   reference the new module path.
5. **Update `specification.md §4.4` and §5** to say "render effect" and
   `RenderEffect` consistently.
6. **Preserve apply-to-surface / apply-to-registry semantics** verbatim. The
   methods already at `src/elements.rs` lines 89-110 are the right shape (one
   trait, two convenience entry points); Phase B moves them, does not
   redesign them.
7. **Preserve existing effect tests** at `src/elements.rs` lines 2626-3727
   when splitting. Convert any `TerminalEffect::` mentions in tests as part
   of the rename pass.

### Exit criteria

- `RenderEffect` and `EffectElement` live in `src/elements/effect.rs` (or
  equivalent) with module-level rustdoc stating the data-only contract.
- `cargo check` and `cargo test` pass against the local tui-kit and against
  c4tui's path-dep build (the latter touches no element types so should be a
  no-op).
- The five data-only constraints have been verified by reading the new enum:
  derives + no `Box<dyn _>` + no `Fn` + no `Arc<Mutex<_>>` live-state + no
  ambient access. The compile-only round-trip test passes.
- `architecture.md §8` and `specification.md §4.4`/§5 use `RenderEffect`
  consistently. README spot-checked for stale `TerminalEffect` language.
- No "graceful migration" shim or deprecated alias remains.

### Non-goals

- No frame transport, no wire schema, no client helper.
- No serde implementation on `RenderEffect` itself. The data-only contract
  *enables* serialization; committing to a representation is Phase G-territory
  and explicitly deferred.
- No new effect variants. Adding `#[non_exhaustive]` is preserved; the variant
  set is unchanged from the current enum.
- No retained-widget changes. Phase C handles those.

### Risks and mitigations

- **Risk: extraction touches the test module's `EffectProbeElement` and
  `ToggleEffectProbeElement` (lines 2786-2929) and their assertions
  (lines 3018+).** *Mitigation:* extract before renaming. Land the move in one
  commit (no behavior change), then rename in a second commit (mechanical
  s/TerminalEffect/RenderEffect/). Two commits keep the diff reviewable.
  **Confidence: HIGH.**
- **Risk: introducing `src/elements/` as a directory module breaks rustdoc
  links or downstream `use tui_kit::elements::*;` patterns.** *Mitigation:*
  inspect `src/prelude.rs` lines 21-23 first; the prelude is the only stable
  re-export surface. Preserve every `pub use` path. The directory move is
  internal and should be invisible to consumers. **Confidence: HIGH.**
- **Risk: a test that relies on `Debug` formatting of `TerminalEffect`
  variants breaks because the Debug output now reads `RenderEffect::...`.**
  *Mitigation:* grep tests for `TerminalEffect::` string-literal assertions
  before renaming (none expected — most assertions use `matches!` or
  destructuring). **Confidence: MEDIUM** (not exhaustively verified).
- **Risk: scope creep — agent decides to "also" split out `Stack` or `Window`
  while the file is open.** *Mitigation:* Phase B is two commits only: extract
  effect module, then rename. The broader split is Phase C, and only if F's
  outcomes justify it.

### What this phase explicitly does **not** decide

- Whether the rest of `elements.rs` gets split further (Phase C).
- Whether `EffectElement` should become a method on `BufferComponent` instead
  of a separate trait (out of scope; not needed for the data-only contract).
- Whether `RenderEffect` should be serializable today (no — only "data-only
  in principle"; serde lands when a real transport pulls it).

## Phase F — c4tui Validation Work (CONCURRENT with B)

**Purpose:** Drive the consumer cleanup that proves which tui-kit surfaces are
durable.

### Pre-conditions and verified state

- c4tui currently has three separate picker types
  (`c4tui/src/picker.rs:ViewPicker`, `c4tui/src/connection_picker.rs:
  ConnectionPicker`, and an inline child-view picker in `c4tui/src/app.rs`)
  and three modal slot lifecycles, per
  `docs/superpowers/plans/2026-05-12-phase-3-navpicker-modal-image-elements.md`
  Decision A-D. **Confidence: HIGH** (verified by grep of `c4tui/src/app.rs`
  lines 4-10, 30-31, 73-74, 312-313).
- `LinkDirectory` does not yet exist in c4tui.
- The image viewport winner (`image_viewport`, per Decision B in the
  superseded Phase 3 plan) is documented but final deletion of the loser
  (`image_box`) belongs to Phase C, not F.

### Dependencies

- Phase A doc alignment (shipped). Phase F does not depend on Phase B's
  rename; c4tui consumes none of the affected types. **Confidence: HIGH.**
- Phase F's individual sub-tasks are independent and can land in any order.
  Each sub-task that lands frees one corresponding Phase C decision.

### Scope (phase-level)

1. **NavPicker consolidation.** Replace `ViewPicker`, `ConnectionPicker`, and
   the inline child-view picker with one `NavPicker<T: NavItem>` over a
   union `NavTarget` enum. Specifics are pre-decided in
   `docs/superpowers/plans/2026-05-12-phase-3-navpicker-modal-image-elements.md`
   Decisions A-D; use them or supersede them per-task. Result: one picker
   type, one render path, one keymap, one modal slot.
2. **Modal handling unified.** One `Modal` trait, one `ActiveModal` enum,
   one `render_modal`/`close_modal` pair on the terminal backend. Dialog stays
   out (one-shot semantics), per Decision D.
3. **Single image-viewport path wired.** `ImageViewport` is the survivor.
   Confirm c4tui's `view.rs` and `terminal.rs` rely on it (`grep` returns 11
   types imported in `view.rs` from `image_viewport` and zero from
   `image_box`); when Phase C deletes `image_box`, c4tui needs no changes.
4. **LinkDirectory shipped.** Implement the keyboard-first link-navigation
   surface. Treat it as a `NavPicker<LinkCandidate>` consumer if it fits;
   otherwise document the divergence in c4tui only.
5. **Command/effect duplication collapsed.** Whatever Phase 3 (older plan)
   left half-converted around picker open/close commands and effects, finish.
6. **View catalog / render cache / viewport state split.** Once
   picker and LinkDirectory dependencies are narrow, split these as
   documented in the fresh plan; do not block on it before NavPicker lands.
7. **Each landing sub-task reports back to Phase C.** When NavPicker lands,
   record whether it pulled in `Stack`, `Window`, `Modal`, `Overlay`, or
   `KeyScope` from tui-kit. When `LinkDirectory` lands, do the same. The
   answer drives Phase C deletion decisions.

### Exit criteria

- c4tui has one picker (`NavPicker<T>`) and one modal abstraction (`Modal`
  trait + `ActiveModal`).
- `LinkDirectory` is shipped and used by c4tui's keyboard-first navigation
  path.
- Sub-tasks have produced a written list (recorded in this plan or in a
  follow-up handoff under `docs/superpowers/handoffs/`) of which tui-kit
  retained-widget types c4tui actually depends on after the cleanup.
- View catalog / render cache / viewport state split is either complete or
  scoped to a follow-on c4tui-only plan.

### Non-goals

- No tui-kit deletions inside Phase F. Phase F *records what's consumed*; the
  deletions land in Phase C.
- No image-protocol additions (Phase D handles reliability of the existing
  protocol; protocol additions are deferred).
- No live SSH/container testing — operator-owned.

### Risks and mitigations

- **Risk: NavPicker consolidation pulls in retained-widget types from
  tui-kit's `elements`, then Phase C deletes them.** *Mitigation:* Phase F
  reports consumed types back to Phase C explicitly. Phase C does not delete a
  type if F's report says it has a consumer. **Confidence: HIGH.**
- **Risk: c4tui drifts into building UI primitives that should live in
  tui-kit.** *Mitigation:* the Extension Rules in `architecture.md §11`
  already encode the four tests for a new tui-kit surface. Phase F sub-tasks
  apply that rubric when deciding "lift into tui-kit?" vs "keep app-local."
- **Risk: NavPicker breaks tab-order or focus semantics during the
  consolidation.** *Mitigation:* `c4tui/src/app.rs` already has modal scope
  handling (`SCOPE_PICKER`, `SCOPE_CONNECTION_PICKER`, lines 30-31, 312-313);
  the consolidation must preserve those scopes or migrate them deliberately.

## Phase C — Elements Triage (GATED on Phase F outcomes)

**Purpose:** Decide the survivors among the retained-widget types in
`src/elements.rs`, and either trim or split the file accordingly.

### Pre-conditions and verified state

- After Phase B extracts effect types, `src/elements.rs` (or
  `src/elements/mod.rs` if the dir layout is adopted) contains: `Element`
  marker trait (line 39), `ContainerElement` (line 44), `Padding` (line 164),
  `ElementBorder` (line 238), `Text` (line 307, with `TextOverflow` and
  `render_text_lines`), `Panel` (line 465), `Stack` (line 615), `ScrollY`
  (line 935), `Focusable` (line 1055), `KeyMapped` (line 1148), `Padded`
  (line 1259), `Bordered` (line 1344), `Window` family (lines 1425-2075,
  including `WindowRepaintPolicy`, `WindowRenderStats`,
  `WindowLifecycleEvent`, `WindowHooks`, `WindowFocusScope`, `WindowChrome`,
  and `Window<E>` itself), `KeyScope`/`KeyScopeResolver` (lines 2078-2184),
  `ImageViewportElement` (line 2188), `Modal` (line 2311), `Overlay`
  (line 2431). **Confidence: HIGH** (grep verified).
- **c4tui consumes none of these today.** Zero hits across
  `/Users/coleshaffer/Projects/c4tui/src/` for `Element`, `Bordered`,
  `Padded`, `Panel`, `Stack`, `Window`, `Modal`, `Overlay`, `Text`,
  `Focusable`, `KeyMapped`, `ScrollY`, `ImageViewportElement`. **Confidence:
  HIGH** (grep verified).
- The tests in lines 2626-3727 *internally* consume `Stack`, `Window`,
  `Modal`, `Overlay`, and the effect probe types. Some of these tests encode
  durable invariants (area-transforming effect forwarding, grouped placement
  teardown). They are not automatically deletion candidates if the surrounding
  type is deleted; first determine whether the *invariant* belongs in the
  surviving code.

### Dependencies

- Phase B (effect module extracted, rename complete). Without B, splits and
  deletions touch the load-bearing effect types and become risky.
- Phase F (consumer-driven sub-tasks landed). Each F sub-task that lands
  produces evidence about which retained types have a real consumer.

### Scope (phase-level)

1. **Confirm the keep-list.** Per direction doc lines 130-141, the durable
   pieces are `BufferComponent` (in `component.rs`, not here), `Element`,
   `RenderEffect` (already extracted in B), `EffectElement` (B),
   `ElementExt`, plus cheap sugar: `Text`, `Padding`, `ElementBorder`,
   `Padded`, `Bordered`, `ScrollY`, `Focusable`, `KeyMapped`. Phase C keeps
   these unless a real consumer disagrees.
2. **Evaluate the speculative set.** `Panel`, `Stack`, `Window`,
   `Modal`, `Overlay`, `WindowChrome`, `KeyScope`, `KeyScopeResolver`,
   `ImageViewportElement`. For each: check Phase F's report. If no consumer
   in c4tui post-cleanup and no test encoding a hard render/effect invariant
   that cannot live elsewhere, delete decisively (per
   `2026-05-13-library-author-direction.md` lines 144-156). If a consumer
   pulled it in, keep and document the consumer.
3. **For invariants worth keeping:** if a test in lines 2626-3727 encodes
   area-transforming effect forwarding or grouped placement teardown that
   matters even without the retained widget, move that test to assert against
   `EffectElement` plus a minimal in-test fixture, and let the retained
   widget go.
4. **Rationalize file layout.** See the "Honest scope check" section
   immediately below for the recommended split. Execute the split only if the
   keep-list after F warrants it; do not split for its own sake.
5. **Reconcile docs.** Update `architecture.md §2` widget table and
   `specification.md §4.4` to reflect the surviving set. Update
   `src/prelude.rs` re-exports.

### Honest scope check on `src/elements.rs` (3,727 lines)

**Recommendation: split, but only after Phase B and after Phase F reports
which retained widgets survive.** **Confidence: MEDIUM-HIGH.**

Reasoning:

- Production is ~2,625 lines (lines 1-2625). Tests are ~1,100 lines (2626-
  3727). Moving tests alongside the modules they exercise is a trivial win.
- Extracting `RenderEffect + EffectElement + apply_to_*` to
  `src/elements/effect.rs` (Phase B) removes 110 lines of the load-bearing
  contract from the same file as the speculative retained widgets — that
  alone is the most valuable cut. **Confidence: HIGH.**
- The remaining production code splits naturally into four buckets:
  - **Leaf/decoration sugar (keep almost certainly):** `Text`, `Padding`,
    `ElementBorder`, `Padded`, `Bordered`, `ScrollY`, `Focusable`,
    `KeyMapped`, `ElementExt`. ~700 lines. Candidate file:
    `src/elements/decorators.rs`. **Confidence: HIGH.**
  - **Containers (Phase C decides):** `Panel`, `Stack` (with
    `StackDirection`, `StackConstraint`, `solve_stack_lengths`). ~400 lines.
    Candidate file: `src/elements/containers.rs`. Stack carries `ChildElement<M>`
    plumbing (lines 639-679) that is reused by `Overlay`. **Confidence: MEDIUM.**
  - **Window family (Phase C decides):** `WindowRepaintPolicy`,
    `WindowRenderStats`, `WindowLifecycleEvent`, `WindowHooks`,
    `WindowFocusScope`, `WindowChrome`, `Window<E>`, `Modal<E>`. ~750 lines.
    Candidate file: `src/elements/window.rs`. Direction doc explicitly flags
    `Window` as "the natural future seam below transport" — keep iff a
    consumer pulls it. **Confidence: MEDIUM.**
  - **Overlay + KeyScope + ImageViewportElement (Phase C decides):** ~400
    lines. Candidate file: `src/elements/overlay.rs` (with `ImageViewportElement`
    possibly migrating to `src/widgets/` if it survives). **Confidence:
    MEDIUM.**
- **Do not** create a four-way directory split speculatively. The win comes
  from Phase B's extraction, and from Phase C deleting types that have no
  consumer. If Phase F's report leaves only the keep-list plus, say,
  `Stack`, then a two-file split (`effect.rs` + a single trimmed `mod.rs`
  with `decorators.rs`) is enough.

The right scope check answer: **extract `RenderEffect`/`EffectElement` in B,
delete what F shows is dead in C, then split what's left only if the
remaining file is still hard to read.** Splitting before deletion is busywork.

### Exit criteria

- `src/elements.rs` (or its successor directory) contains only the retained
  surface plus tests that encode real invariants. The file or directory
  totals ≤1,200 production lines, or every retained widget has a named
  consumer (in c4tui, in `widgets/`, or in a test asserting a render/effect
  invariant).
- The retained set is reflected in `architecture.md §2`, `specification.md
  §4.4`, and `src/prelude.rs`.
- Deletions are decisive — no "graceful migration" aliases, no deprecated
  re-exports.
- No tui-kit warning or test is added that exists only to support a deleted
  retained widget.

### Non-goals

- No new retained-widget machinery. Phase C *only* shrinks.
- No expansion of `Element` into a retained component tree runtime. The
  direction doc forbids this (lines 144-156).
- No protocol design.

### Risks and mitigations

- **Risk: deleting `Window` removes the natural future seam for transport.**
  *Mitigation:* the direction doc already says `Window` should not define the
  transport boundary; the seam is below at `BufferComponent + RenderEffect`.
  Deleting `Window` removes a piece of speculative local convenience, not a
  load-bearing future commitment. **Confidence: HIGH.**
- **Risk: a test in lines 2626-3727 turns out to encode an invariant we want
  even after deleting its container widget.** *Mitigation:* during deletion,
  read each test that touches a removed type. If the assertion is about
  `EffectElement` semantics (forwarding through area transforms, teardown
  composition), preserve it as an assertion on a minimal in-test fixture
  derived from `EffectProbeElement` (already at lines 2786-2841).
  **Confidence: MEDIUM.**
- **Risk: Phase F under-reports its consumed set, and Phase C deletes a type
  that c4tui will need next quarter.** *Mitigation:* the rip-and-replace
  policy combined with SemVer means re-adding is cheap; the cost of keeping a
  speculative widget around is doc clutter and an implied API contract.
  Bias toward deletion when F's report is silent. **Confidence: MEDIUM-HIGH.**

## Phase D — Image Path and SSH/Container Reliability

**Purpose:** Make the current Kitty image path robust in the local/SSH/
container environments the operator actually uses, with operator-runnable
smoke checks.

### Pre-conditions and verified state

- `ImageSurface` trait + `KittyImageRegistry` + `NoopImageSurface` +
  `ImageSurfaceRegistry` are in `/Users/coleshaffer/Projects/tui-kit/src/image.rs`
  (lines 1-80 for the trait + protocol enums; full module is 652 lines).
  The lifecycle (`ensure_loaded` → `place` → `delete_image_placement` /
  `delete_placement` / `delete_all_placements` → `forget_all`) is documented
  at lines 9-22. **Confidence: HIGH.**
- `ImageViewportWidget` is the surviving image widget (`image_viewport.rs`,
  940 lines). `image_box.rs` (907 lines) is the loser per Phase 3 plan
  Decision B; its deletion belongs to Phase C, not D.
- `tty.rs` has metric/probe helpers (lines 1-69); `terminal.rs` (292 lines)
  wraps lifecycle including image flushing.
- The operator boundary is unambiguous: live terminal + real SSH + real
  container tests are operator-only (direction doc lines 176-186, fresh plan
  line 24).

### Dependencies

- Phase B (uses `RenderEffect` vocabulary; tests assert against the renamed
  type).
- No dependency on F or C; D is parallelizable with both.

### Scope (phase-level)

1. **Verify graceful degraded behavior.** Confirm `NoopImageSurface` returns
   from every `ImageSurface` method without panicking, and that the
   `ImageBackendPreference::Disabled` path is exercised by a test. If
   absent, add one.
2. **Document required terminal environment.** `TERM`, `COLORTERM`,
   terminfo entries Kitty graphics relies on, and the relevant
   `tty.rs` probe outputs. Land this as a section in `architecture.md` (new
   §7.1 or appended to §8).
3. **Add tests for image-lifecycle and placement math** that do not require
   a real terminal. Use `MockImageSurface` + `MockImageCall` (already in
   `testkit.rs`, referenced from `widgets/grid.rs` line 685 and
   `widgets/image_box.rs` line 575). Cover at least: load-then-place,
   place-then-resize-area, teardown-then-place, repeated place with stable
   placement id, and explicit `forget_all` cycle.
4. **Author operator smoke-test scripts/checklists** under
   `docs/superpowers/handoffs/` (new file:
   `2026-05-XX-image-smoke-checklist.md` or similar). Cover: local
   Kitty/WezTerm, SSH (terminfo + `TERM` propagation), SSH-into-container
   (PTY + Kitty passthrough), `docker exec` (or equivalent). Each section is
   an explicit operator script with expected outputs.
5. **Record known terminal/container passthrough edge cases.** As the
   operator runs the scripts, surface findings into the same checklist file
   under a "Known passthrough edge cases" section.

### Exit criteria

- `cargo test` covers the image lifecycle without a live terminal. Test
  inventory documented in `architecture.md §10`.
- Required env vars / terminfo entries are documented.
- Operator smoke checklist exists. Operator has run it once (initial
  baseline) and recorded results under `docs/superpowers/handoffs/`.
- Graceful degraded path is asserted by a test, not just claimed by docs.

### Non-goals

- No new image protocol. Sixel/iTerm/etc are deferred per the spec's §3
  non-goals.
- No claim of "SSH/container verified" without operator sign-off. Agents
  write the scripts; operator runs them.
- No automated CI step that runs the smoke scripts (would require a real
  terminal stack).

### Risks and mitigations

- **Risk: documenting envs that drift from reality.** *Mitigation:* every
  documented env var should be referenced by a `tty.rs` probe or by
  `terminal.rs`/`image.rs` runtime behavior. If no code reads it, do not
  document it. **Confidence: HIGH.**
- **Risk: smoke checklist becomes a write-only doc.** *Mitigation:* tie the
  plan-level Exit criterion (#6) to operator sign-off recorded as a handoff
  file. No sign-off, no Phase D completion.
- **Risk: tests use mocks that drift from the real Kitty protocol.**
  *Mitigation:* the `ImageSurface` trait is the contract; mocks implement the
  same trait. Drift surfaces as a trait-method gap, which is a compile error.
  **Confidence: HIGH.**

## Phase E — Testkit Hardening

**Purpose:** Make the future transport-safe contract testable without
transport, with helper assertions for render-effect shape.

### Pre-conditions and verified state

- `src/testkit.rs` (617 lines) currently exposes `MockImageSurface`,
  `MockImageCall`, and `DeterministicScheduler` (the latter has a parity test
  at `tests/parity.rs` lines 1-80). `testkit` does **not** reference any
  `elements`/effect type today. **Confidence: HIGH** (grep verified).
- `RenderEffect` (after Phase B) is the natural target for testkit
  assertions: it is the data the future transport would carry.

### Dependencies

- Phase B (testkit asserts against the renamed, extracted `RenderEffect`).
- Phase D for image-side fixtures (mock image surface is already in
  `testkit`; Phase D may extend it).
- No dependency on F or C.

### Scope (phase-level)

1. **Strengthen mock render/image surfaces.** Audit `MockImageSurface` for
   gaps against the `ImageSurface` trait; ensure every method records a
   `MockImageCall` so test assertions can reason about lifecycle.
2. **Add helper assertions for `RenderEffect` sequences.** A small set of
   matchers for "this Vec<RenderEffect> contains a PlaceImage with these
   options," "this sequence ends with a DeleteImagePlacement for image X,
   placement Y," "this teardown sequence covers every placement we expect."
   Keep them as plain functions or simple structs, not macros.
3. **Keep pure buffer rendering tests easy to write.** Confirm `Cached`,
   `BufferComponent::render_buffer`, and the buffer cell helpers from
   ratatui give a one-call assertion pattern. If not, add a thin testkit
   helper.
4. **Add golden/snapshot fixtures only where they reduce ambiguity.**
   Bias toward structural assertions over goldens. If a golden is needed,
   keep it in `tests/` not `src/testkit.rs`, and document the regeneration
   command.
5. **Document when to use tui-kit `testkit` vs c4tui's fake backend.** Add a
   short section to `architecture.md §10` (testing architecture). The line:
   testkit asserts library-level invariants; c4tui's fake backend asserts
   app-level wiring.

### Exit criteria

- Testkit exposes mock render/image surfaces and `RenderEffect` assertion
  helpers usable from `tests/` and from c4tui's tests.
- `architecture.md §10` says when to use testkit vs the c4tui fake backend.
- Existing testkit users (`tests/parity.rs`, `widgets/grid.rs` tests,
  `widgets/image_box.rs` tests pre-deletion or `widgets/image_viewport.rs`
  tests post-deletion) continue to compile and pass.

### Non-goals

- No transport. No serialization helpers beyond `Debug + PartialEq` round-
  trip checks.
- No async / runtime-specific helpers. Direction doc forbids requiring an
  async runtime.
- No event-loop test harness in tui-kit; that's c4tui's territory.

### Risks and mitigations

- **Risk: testkit grows helpers c4tui never calls, becoming speculative
  surface.** *Mitigation:* every new testkit helper either has a c4tui test
  call site or a tui-kit `tests/` call site within the same phase that adds
  it. No helper ships without a consumer. **Confidence: HIGH.**
- **Risk: assertion helpers leak ambient state (global counters, statics).**
  *Mitigation:* the data-only contract on `RenderEffect` already forbids
  this on the production side; mirror it in testkit. **Confidence: HIGH.**

## Operator-owned vs agent-owned boundary

Lifted verbatim from the direction doc and pinned here so executors do not
re-derive it:

- **Agent-owned:** code, tests, docs, plan files, handoff stubs, mocks,
  unit/integration tests, `cargo check` / `cargo test` runs in-session.
- **Operator-owned:** live terminal smoke tests (local Kitty/WezTerm), real
  SSH tests, real container tests, `docker exec` PTY tests, WezTerm config
  integration, verification of Kitty graphics behavior in actual terminal
  stacks. Sign-off lands as a handoff file under
  `docs/superpowers/handoffs/`. Without that file, no Phase D Exit criterion
  about real-environment behavior can be marked done.

Agents must not claim live SSH/container verification in commit messages,
plan updates, or handoff files. If the operator has not signed off, the
status is "scripted, awaiting operator."

## Deferred pulls (reaffirmed, not in active scope)

- `tui-kit-agent`.
- Full frame transport.
- Remote renderer protocol.
- `dgksh` as shell/session supervisor.
- Persistent discovery/caching/multiplexing.
- Non-terminal renderers.

Reopen only with a concrete use case that the library + ordinary SSH/
container reliability cannot solve.

## Working rules

- Phase B and Phase F may proceed in parallel.
- Phase C does not start its deletion sub-tasks before Phase F has reported
  consumed types for at least the affected sub-area.
- Phase D operator steps queue against operator availability; D's
  library-side work (tests, env documentation, scripts) does not.
- Phase E can interleave with B/D/F; do not block on F.
- Every phase commits in small, scoped pieces. Phase B is two commits
  (extract, then rename). Phase C deletions land per-type, not in one
  catch-all.
- Prefer deletion over compatibility shims while the crate is early.
- Every new public surface needs a named consumer or a test proving a
  terminal, layout, render-effect, or lifecycle invariant. (Mirrors
  `architecture.md §11`.)

## Operator sign-off requested before Phase B starts

1. **Confirm the supersede.** This doc takes precedence over
   `2026-05-13-fresh-library-author-plan.md` for execution; the fresh plan
   remains as strategic history. Acceptable?
2. **Confirm the new dependency graph.** Specifically: Phase F runs
   concurrent with B; Phase C is gated on F outcomes per-sub-area, not on a
   single F completion signal. Acceptable?
3. **Confirm the Phase B two-commit shape.** Commit 1: extract effect module
   with no rename. Commit 2: rename `TerminalEffect → RenderEffect` and
   `terminal_effects → render_effects` across the crate. Anything that
   should land in commit 1 (e.g. the new module-level rustdoc) vs commit 2?
4. **Confirm Phase C's deletion bias.** Default-to-delete when Phase F is
   silent on a retained widget, with SemVer re-add as the fallback. Or
   default-to-keep until F explicitly says "unused"? Plan currently bets on
   default-to-delete; change if you want default-to-keep.

After sign-off on these four, the Phase B bite-sized plan is the next
artifact to produce.
