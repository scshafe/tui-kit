# Phase D — Image Path and SSH/Container Reliability (Bite-Sized Plan)

**Status:** ACTIVE · 2026-05-15
**Parent:** [`2026-05-14-revised-library-author-implementation-plan.md`](./2026-05-14-revised-library-author-implementation-plan.md)
**Predecessor in graph:** Phase B (shipped), Phase E (shipped).
**Concurrent with:** c4tui's ongoing Phase F.
**Independence:** Does not block, and is not blocked by, Phase C or Phase F.

## Goal (lifted from parent plan §"Phase D")

Make the current Kitty image path robust in the local/SSH/container
environments the operator actually uses, with operator-runnable smoke checks.
Library-side: lock the data-flow lifecycle in tests using the `MockImageSurface`
+ `RenderEffect` helpers from Phase E. Operator-side: ship a runnable
smoke-test checklist that the operator executes against real terminals and
records results for.

## Why this is the next on-path work

Phase E shipped `MockImageSurface`, `find_place_with_placement_id`, and
`assert_teardown_covers`. Phase D was deliberately sequenced after E so it
could use those helpers without retrofitting. The parent plan's dependency
graph (`B → {D, E}`, `D ⊥ E`) confirms D is unblocked. **Confidence: HIGH.**

## Pre-conditions and verified state

- **No env-var reads in tui-kit.** Grep for `env::var` / `std::env` across
  `src/` returns zero results. The parent plan's scope item 2 ("document
  required env vars") therefore collapses: there are no env vars to document.
  What does exist is a **terminal-protocol** requirement (Kitty graphics
  responder behavior, alt-screen + raw-mode lifecycle) — that's what should
  be documented instead. **Confidence: HIGH.**
- **Graceful degraded path is already partially covered.**
  - `src/image.rs:600-622` `noop_surface_accepts_full_lifecycle_without_io`
    asserts every `NoopImageSurface` method works without panicking.
  - `src/image.rs:582` `surface_registry_selects_disabled_backend_as_noop`
    asserts the `ImageBackendPreference::Disabled` selection produces a
    `NoopImageSurface`.
  - **Gap:** the registry's full lifecycle through `ImageBackendPreference::Disabled`
    (load → place → delete → forget_all via `ImageSurfaceRegistry`) is not
    asserted. Production code routes through the registry, not directly to
    `NoopImageSurface`. Closing this gap is a one-test addition.
  **Confidence: HIGH** (read-verified).
- **`KittyImageRegistry::place` deletes a pre-existing placement before
  re-placing** (`src/image.rs:410-417`). This is a production invariant —
  re-placing with the same `placement_id` is idempotent on placement state.
  `MockImageSurface` records every call verbatim, so the mock-level test
  documents the data flow without modeling Kitty's internal deletion. The
  registry-level behavior is tested implicitly by the escape-formatting
  tests at `src/image.rs:625-650`.
- **`ImageBackendPreference` includes a `KittyOnly` strict variant and a
  `Disabled` degraded variant.** The `AutoDetect` variant exists but, with
  only Kitty implemented, currently picks Kitty when ordered. No protocol
  probing happens at runtime. **Confidence: HIGH** (read-verified).
- **Phase E helpers ship in `src/testkit.rs`:** `render_to_buffer`,
  `find_place_with_placement_id`, `assert_teardown_covers`,
  `MockImageSurface`, `MockImageCall`. Phase D's lifecycle tests will use
  `MockImageSurface` + `MockImageCall` directly (the new assertion helpers
  are render-effect-shaped, not call-sequence-shaped).

## Non-goals (inherited)

- No new image protocol (Sixel, iTerm2, etc — deferred per spec §3 non-goals).
- No claim of "SSH/container verified" without operator sign-off. Agents
  write scripts; operator runs them.
- No automated CI step that runs the smoke scripts (would require a real
  terminal stack).
- No protocol probing / runtime detection beyond what already exists.

## Commit plan (3 commits, behavior-preserving)

### Commit 1 — `image`/`testkit`: lifecycle tests for the named scenarios

**Files:** `src/image.rs` (new tests), `src/testkit.rs` (potentially) — no
production code changes.

Add `cargo test` coverage for the data-flow lifecycle scenarios the parent
plan names:

1. **Load-then-place** — `ensure_loaded` → `place`. Assert exact `MockImageCall`
   sequence.
2. **Place-then-resize-area** — `place(opts_a)` → `place(opts_b)` with the
   same `placement_id` but different `cell_cols`/`cell_rows`. Assert the
   mock records both places verbatim (production `KittyImageRegistry` would
   internally delete-then-replace; the mock captures the *requested* sequence,
   which is the contract the apply path commits to).
3. **Teardown-then-place** — `place(opts)` → `delete_image_placement(image,
   placement)` → `place(opts)`. Assert sequence.
4. **Repeated place with stable placement id** — `place(opts)` × 3 with
   identical args. Assert the mock records three places (caller's
   responsibility, not the surface's, to coalesce).
5. **`forget_all` cycle** — `ensure_loaded` → `place` → `forget_all` →
   `ensure_loaded` → `place`. Assert both `EnsureLoaded` calls are recorded
   (loaded set has been forgotten and must be repopulated).
6. **`ImageBackendPreference::Disabled` registry lifecycle** — construct
   `ImageSurfaceRegistry::from_preference(Disabled)`, run the full lifecycle
   (`ensure_loaded`, `place`, `delete_image_placement`, `delete_placement`,
   `delete_all_placements`, `forget_all`) without panicking. Closes the gap
   identified above.

These tests live in `src/image.rs`'s existing test module (alongside
`noop_surface_accepts_full_lifecycle_without_io`). Test names follow the
existing convention `<subject>_<verb>_<expected>`.

**Verification:** `cargo test --quiet` green; `cargo clippy --all-targets`
clean; `cargo fmt --check` clean.

**Rationale:** Locks the call-sequence invariants of the image lifecycle in
behavior tests. Future protocol additions or refactors that change the
sequence will fail these tests. Closes parent-plan exit criterion #4
("graceful degraded path is asserted by a test, not just claimed by docs")
and parent Phase D exit criterion #1 (`cargo test` covers image lifecycle
without a live terminal).

### Commit 2 — `architecture.md`: terminal-protocol requirements section

**File:** `architecture.md`.

Append (or sharpen) §7 "Terminal Boundary" or §8 "Render Effects and Image
Lifecycle" to capture the terminal-protocol requirements for Kitty graphics
support. The section should answer four questions a reader needs:

1. **What does Kitty graphics require from the terminal stack?** Kitty
   graphics protocol response handling (the responder loop), `\x1b_G...
   \x1b\\` APC escape support, alt-screen + raw-mode compatibility,
   cursor-positioning support for `place_at`.
2. **What does tui-kit assume about the alt-screen + raw-mode lifecycle?**
   Lifecycle is owned by `Terminal::enter_with_config`; raw mode is enabled
   before any image escape is written; alt screen is exited and image data
   is cleaned up on `Drop`. Tests must not enter a real alt screen.
3. **What env vars or terminfo entries does tui-kit consult?** *None.*
   Document this explicitly — the library does not probe `TERM`,
   `COLORTERM`, or any other env var. Backend selection is via
   `TerminalConfig::image_backend` (or its preset constructors), and the
   only runtime detection is "are we a TTY?" via
   `crate::tty::stdout_is_terminal`.
4. **Where does the Kitty escape vocabulary live in code?** Point at the
   `transmit_png`, `kitty_place_escape`, and `kitty_delete_placement_escape`
   helpers in `src/image.rs`, and at the responder/cursor-positioning
   utilities in `src/tty.rs`.

The section also updates `architecture.md §10`'s testkit list to point at
the new lifecycle tests added in Commit 1 (parent Phase D exit criterion #1
asks for the test inventory to be documented in §10).

**Verification:** spot-check that referenced symbols (`transmit_png`,
`kitty_place_escape`, etc.) still exist by file:line. No code changes.

**Rationale:** Replaces the parent plan's "document required env vars" item
with an honest accounting: tui-kit reads zero env vars. The documentation
target is the *terminal-protocol* requirements that produce reliable behavior
in a real terminal, plus the assumptions tui-kit makes about the lifecycle
it owns. Closes parent Phase D exit criterion #2 ("required env vars /
terminfo entries are documented" — answered: there are none, here's what
matters instead).

### Commit 3 — Operator smoke-test checklist file

**File (new):** `docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`.

Author an operator-runnable checklist with four sections:

1. **Local Kitty / WezTerm** — commands to verify the cargo example renders
   an image inline; expected visual outcome; what failure looks like.
2. **SSH (terminfo + `TERM` propagation)** — `ssh -t user@host` command,
   precondition checks (terminfo on the remote, `TERM` value preserved),
   expected behavior, failure modes (image escapes printed as literal text,
   alt-screen not entered, mouse capture stuck).
3. **SSH into container (PTY + Kitty passthrough)** — `ssh -t user@host
   docker exec -it container ...` or equivalent; the question of whether
   the container has terminfo; what `TERM` survives the layers.
4. **`docker exec` (local PTY)** — `docker exec -it container ...` smoke;
   expected behavior; how to identify the bad layer if it fails.

Each section is structured as:

- **Setup** — environment prep (no surprises).
- **Run** — exact command to execute.
- **Expected** — what the operator should see / record.
- **Failure modes** — observable symptoms and likely cause.

The file also includes:

- A **"Known passthrough edge cases"** section, pre-populated with what the
  code structure suggests (e.g., "Kitty escapes are written via
  `io::stdout().lock()` per-call in `transmit_png`; PTY-layer-induced
  reordering would interleave PNG bytes from concurrent `transmit_png`
  calls — tui-kit currently uses a single image_id-uniqueness lock via the
  `loaded` set, not a write-side lock") and a clear template for the
  operator to add observed cases.
- A **"Sign-off"** block — date, operator name, terminals tested, summary
  of results. Parent plan exit criterion #3 requires this file to exist
  *and* be signed off after the operator runs the checklist; this commit
  ships the unsigned scaffold.

**Verification:** the file is markdown-valid; commands in it use absolute
paths or `cd /Users/coleshaffer/Projects/tui-kit` prefixes; expected
outputs are concrete (visible image, specific escape sequence visible in
output, etc.).

**Rationale:** Operator sign-off is gated on running the checklist, but the
checklist itself must exist first. Commit 3 ships the checklist; sign-off is
the operator's separate handoff that lands when they record results. Until
then, the parent plan's Phase D exit criterion #3 is "scripted, awaiting
operator."

## Plan-level Exit criteria (Phase D, from parent plan)

After all three commits land:

- ✅ `cargo test` covers the image lifecycle without a live terminal. Test
  inventory documented in `architecture.md §10`. (Commits 1 + 2)
- ✅ Terminal-protocol requirements are documented (the env-var inventory
  is honestly "none read"; the real requirements are spelled out). (Commit 2)
- ⏳ Operator smoke checklist exists. Operator has run it once (initial
  baseline) and recorded results under `docs/superpowers/handoffs/`. **The
  checklist ships in Commit 3; operator-run sign-off is a separate handoff
  that lands when the operator records results.** (Commit 3 partial)
- ✅ Graceful degraded path is asserted by a test, not just claimed by docs.
  (Commit 1: `disabled_registry_full_lifecycle_does_not_panic` or
  equivalent.)

3 of 4 closed by this plan; the 4th is "scripted, awaiting operator" until
the operator runs and signs.

## Working rules (inherited)

- No claim of "SSH/container verified" in commit messages or in the handoff
  until the operator has signed off.
- Every commit is small, scoped, behavior-preserving (no production code
  changes in this plan).
- No new public surface — only new tests, new docs, new handoff scaffold.
- Commit messages follow tui-kit's lowercase-imperative style.

## Risks and mitigations

- **Risk: documenting protocol requirements that drift from code.**
  *Mitigation:* every documented behavior must point at a file:line in
  `src/`. If no code implements it, do not document it. **Confidence: HIGH.**
- **Risk: smoke checklist becomes a write-only doc.** *Mitigation:* parent
  plan's exit criterion #3 is gated on an operator-signed handoff. No
  sign-off, no Phase D completion claim.
- **Risk: lifecycle tests document behavior the production registry doesn't
  actually preserve (e.g., re-place semantics).** *Mitigation:* tests
  assert at the `MockImageSurface` boundary, which captures the *requested*
  sequence. Production registries (`KittyImageRegistry`) may collapse,
  reorder, or augment internally — that's the registry's prerogative. If a
  future test wants to assert the production-registry-specific behavior
  (e.g., "place-then-place emits a delete escape"), it should be a separate
  test that observes the escape output, not the lifecycle data flow.
- **Risk: scope creep — agent decides to "also" add a Sixel protocol
  scaffold while the file is open.** *Mitigation:* Phase D is library-
  side reliability of the *existing* Kitty path. No new protocols, no new
  surface. Phase D commits 1–3 strictly.

## What this plan explicitly does **not** decide

- Whether `KittyImageRegistry` should grow runtime probing (e.g.,
  responder-loop detection of `\x1b_G... \x1b\\` support). Deferred —
  current strict/auto-detect/disabled is enough for the operator-tested
  environments.
- Whether to add a Sixel or iTerm2 protocol implementation. Out of scope.
- Whether to extract `MockImageSurface` lifecycle-helpers into testkit (e.g.,
  a `assert_image_loaded(surface, image_id)` matcher). If a real consumer
  emerges in Phase D's tests, it lives in testkit. Otherwise, direct
  `MockImageCall` slice comparisons are enough.
