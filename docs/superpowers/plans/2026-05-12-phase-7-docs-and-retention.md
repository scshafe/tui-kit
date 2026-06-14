# Phase 7 - Documentation and Public-Surface Retention

**Status:** ACTIVE - 2026-06-13
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` Phase 7.
**Predecessors:** Phases 1-6 are complete and committed in both repos.

## Goal

Lock the post-Phase-6 public contract into docs and add a small retention audit
so new tui-kit public types must either have a real consumer or a documented
reason to remain.

## File Structure

### tui-kit

- `scripts/audit-public-surface.sh` - grep-based public type retention audit.
- `docs/public-surface-retention.txt` - documented retention reasons for public
  types that are architectural primitives rather than direct c4tui type names.
- `.github/workflows/ci.yml` - run the audit before the c4tui consumer gate.
- `README.md`, `specification.md`, `architecture.md` - reconcile public
  surface, `image_viewport`, `RenderEffect`, and local/future renderer wording.
- `src/component.rs`, `src/lib.rs` - fix stale module docs.
- `docs/testing-patterns.md` - cross-repo testing seam guide.

### c4tui

- `README.md`, `specification.md`, `architecture.md` - reconcile with
  `NavPicker`, `ActiveModal`, `LinkDirectory`, and the Phase 6
  `ViewCatalog`/`RenderCache`/`ViewportState` split.

## Tasks

- [x] **Task 1 - Retention audit script**
  - Add a POSIX shell script that enumerates `pub struct`, `pub enum`,
    `pub trait`, `pub type`, and `pub const` names in `tui-kit/src`.
  - Exclude private implementation details and generated/build output.
  - Search `/Users/coleshaffer/Projects/c4tui/src`, `tui-kit/tests`, and
    `tui-kit/examples` for non-definition references.
  - Read `docs/public-surface-retention.txt` for documented exceptions.
  - Exit non-zero and print missing entries when a public item has no consumer
    and no documented retention reason.
  - Run:
    `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
    and require an empty missing set.

- [x] **Task 2 - CI wiring**
  - Add the audit to `tui-kit/.github/workflows/ci.yml` after `cargo doc`.
  - In CI, check out `c4tui` before the audit so the script can inspect the
    downstream consumer without modifying it.
  - Keep the existing c4tui consumer gate intact.

- [x] **Task 3 - tui-kit docs reconciliation**
  - Update `README.md` and `specification.md` only where they diverge from
    current code.
  - Update `architecture.md` module map from "image box" to `image_viewport`.
  - Describe `renderer` work as future in Phase 7 docs; Phase 8 will add the
    actual module.
  - Fix stale source docs that still name removed `ViewPicker` APIs.

- [x] **Task 4 - c4tui docs reconciliation**
  - Rewrite architecture sections that still mention removed `ViewStore`.
  - Describe the three owners:
    `ViewCatalog`, `RenderCache`, and `ViewportState`.
  - Replace sketched LinkDirectory wording with the implemented
    `LinkCandidate` fields and `NavPicker<LinkCandidate>` path.
  - Ensure active docs link to
    `docs/archive/historical-plan-phase-0-6.md` only as historical context.

- [x] **Task 5 - Image pipeline diagram**
  - Add a concrete SVG -> PNG -> Kitty registry -> placement diagram to
    `c4tui/architecture.md`.
  - Name the owning code at each stage:
    `RenderPool`/`RenderCache`, `TerminalSession`/`Terminal`, and
    `ViewportState`/`ImageViewportWidget`.

- [x] **Task 6 - Testing-patterns doc**
  - Add `tui-kit/docs/testing-patterns.md`.
  - Cover `tui_kit::testkit` for library-level buffer/effect/scheduler tests.
  - Cover `c4tui::backend::FakeTerminalBackend` for app-level terminal
    lifecycle and modal/render orchestration tests.
  - Include one short worked example for each seam and link the doc from both
    repositories' testing sections.

- [x] **Task 7 - Verification**
  - In `tui-kit`:
    - `cargo fmt --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
    - `cargo doc --no-deps`
    - `scripts/audit-public-surface.sh --c4tui /Users/coleshaffer/Projects/c4tui`
  - In `c4tui`:
    - `cargo fmt --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
  - Re-read the changed docs for stale mentions of `ImageBox`, `image_box`,
    and current-roadmap references to the archived historical plan.
