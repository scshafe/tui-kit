# Phase 5 — LinkDirectory Implementation Plan

**Status:** ACTIVE · 2026-05-16
**Repo for code:** `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` §543-648.
**Predecessors:** c4tui Phases 3 and 4 shipped through `bb7d4a0`.

## Goal

Ship the keyboard-first link directory as the primary "linked from here"
navigation surface. It must reuse the existing `NavPicker` + `ActiveModal`
infrastructure rather than adding another picker or modal lifecycle.

## Task 1 — Pure Candidate Computation

Create `c4tui/src/link_directory.rs` with:

- `LinkKind`;
- `LinkCandidate`;
- `collect_link_candidates(store, current, pinned_element)`.

Candidate families:

- detail links from every element-visible child/detail link on the current view;
- connection links for the pinned element, when present;
- the current view's legend/key link, when present.

Tests cover mixed detail/connection/legend candidates, no-pinned behavior, and
dedupe/sort stability.

## Task 2 — NavPicker Item and State Command

Add `NavTarget::Link` and a `Command::SelectLink(LinkCandidate)` path. Since
`LinkCandidate` implements `NavItem` directly, no wrapper item type is needed.
Selecting a candidate pushes the current view onto breadcrumbs, switches to the
target view, and pins `pinned_element_after_navigation` when provided.

## Task 3 — Modal and Keymap Wiring

Add `Command::OpenLinkDirectory` and `ModalSpec::Link`. Bind Enter at root to
`OpenLinkDirectory`. The LinkDirectory modal is a `NavPicker<LinkCandidate>` using
the existing `ActiveModal::Nav` path. The older connection picker remains as an
internal/specialized path but is no longer the root Enter behavior.

Integration tests open LinkDirectory from root, select a relationship-backed
candidate, and assert view + pinned-element state. Cancel and empty-directory
paths leave navigation unchanged.

## Task 4 — Docs and UI Text

Update c4tui `README.md`, `architecture.md`, `specification.md`, help text, and
status hints so Enter is documented as the link-directory key and mouse drill is
secondary.

## Verification

Run in `/Users/coleshaffer/Projects/c4tui`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Run in `/Users/coleshaffer/Projects/tui-kit`:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```
