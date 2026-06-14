# Phase 18-23 Implementation Handoff

**Status:** READY FOR IMPLEMENTATION SESSION, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Current completed commits when planning started:**

- `tui-kit`: `8378f80 Document post-Phase 17 architecture handoff`
- `c4tui`: `6e1107d Clean up API hygiene and app tests`

This handoff records that the post-Phase 17 architecture follow-up has been
expanded into detailed implementation plans. It does not implement any phase.

`/Users/coleshaffer/Projects/tui-kit/visual-tests/` remains operator-owned and
must not be touched unless explicitly requested.

## Planning Output

Detailed plans now exist for:

1. `2026-06-14-phase-18-c4tui-runtime-transaction-boundary.md`
2. `2026-06-14-phase-19-c4tui-terminal-rendering-backend-split.md`
3. `2026-06-14-phase-20-image-geometry-canonicalization.md`
4. `2026-06-14-phase-21-c4tui-internal-visibility-hygiene.md`
5. `2026-06-14-phase-22-remote-protocol-typed-framing.md`
6. `2026-06-14-phase-23-test-fixture-architecture-cleanup.md`

Read those files before implementing. Also keep the original context available:

- `2026-06-14-post-phase-17-architecture-followup-handoff.md`
- `2026-06-13-phase-10-code-quality-assessment.md`
- `2026-06-14-phase-17-public-api-test-fixture-hygiene.md`

## Implementation Order

Implement in this order, committing and verifying each phase independently
before starting the next:

1. Phase 18 - c4tui Runtime Transaction Boundary
2. Phase 19 - c4tui Terminal Rendering and Backend Split
3. Phase 20 - Image Geometry Canonicalization
4. Phase 21 - c4tui Internal Visibility Hygiene
5. Phase 22 - Remote Protocol Typed Framing
6. Phase 23 - Test Fixture Architecture Cleanup

## Constraints

- No backwards-compatibility shims, deprecated aliases, or migration layers.
- Preserve current behavior unless a phase plan explicitly calls out a behavior
  change for operator approval.
- Keep edits scoped to the active phase.
- Do not touch `visual-tests/` unless explicitly asked.
- Commit each phase before beginning the next phase.
- Run the verification commands listed in the active phase plan.

## Open Questions Before Phase 18

No blocking operator questions remain before Phase 18.

The Phase 18 plan chooses these names as the implementation target:

- `AppRuntime`
- `RuntimeTransaction`
- `RuntimeIntent`
- `ReloadOrigin`

Phase 18 should preserve existing reload messages, render timing, modal
behavior, focus behavior, and terminal call ordering unless implementation
findings expose a real behavior change that needs approval.

