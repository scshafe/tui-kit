# Phase 14 - c4tui Workspace Module Split

**Status:** COMPLETE - implemented and verified, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.
**Predecessors:** Phase 13 remote transport/session state machine.

## Goal

Split `/Users/coleshaffer/Projects/c4tui/src/workspace.rs` into focused
modules for source resolution, export execution, Structurizr model indexing,
view metadata parsing, and view discovery. Preserve current behavior and keep
`crate::workspace::{...}` as the narrow facade for existing c4tui callers.

This phase does not start Phase 15. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Before this phase, `src/workspace.rs` owns all of these responsibilities in one
file:

- `WorkspaceSource` path resolution;
- `ExportedWorkspace`, temporary export ownership, Structurizr CLI invocation,
  and workspace JSON discovery;
- public workspace model types, relationship indexing, and element metadata;
- Structurizr JSON structs used by both model and view metadata parsing;
- view metadata extraction from workspace JSON;
- SVG discovery, `ViewInfo` construction, key-view wiring, and child-view
  wiring;
- inline tests for resolver, metadata parsing, relationship indexing, and view
  wiring.

## Target File Structure

Create:

- `/Users/coleshaffer/Projects/c4tui/src/workspace/mod.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/source.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/export.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/model.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/metadata.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/discover.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/json.rs`
- `/Users/coleshaffer/Projects/c4tui/src/workspace/tests.rs`

Remove:

- `/Users/coleshaffer/Projects/c4tui/src/workspace.rs`

Modify:

- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-14-phase-14-c4tui-workspace-module-split.md`

## Public API Rules

- `crate::workspace` remains the public facade for current c4tui modules.
- Keep public names currently used by c4tui:
  `WorkspaceSource`, `ExportedWorkspace`, `WorkspaceModel`,
  `ElementMetadata`, `RelationshipMetadata`, `ConnectionCounts`,
  `ElementKind`, `ViewKind`, `ViewInfo`, `resolve_workspace`,
  `export_workspace`, `load_workspace_model`, and `discover_views`.
- Keep internal Structurizr JSON and metadata parser types private to the
  workspace module tree.
- No compatibility aliases, deprecated exports, or migration shims.

## Implementation Tasks

- [x] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [x] Move `WorkspaceSource` and `resolve_workspace` into
  `workspace/source.rs`.
- [x] Move `ExportedWorkspace`, `export_workspace`, and workspace JSON
  detection into `workspace/export.rs`.
- [x] Move public model metadata types, relationship indexes, model loading,
  and element collection into `workspace/model.rs`.
- [x] Move Structurizr JSON structs shared by model and view metadata parsing
  into private `workspace/json.rs`.
- [x] Move `ViewMetadata`, `ViewMetadataEntry`, metadata loading, view pushing,
  and key normalization into `workspace/metadata.rs`.
- [x] Move `ViewInfo`, SVG listing, view discovery, key-view wiring, and
  child-view wiring into `workspace/discover.rs`.
- [x] Move inline workspace tests into `workspace/tests.rs`, preserving
  resolver, metadata parse, relationship index, and child-view wiring coverage.
- [x] Add focused malformed/missing metadata coverage that preserves the
  existing fallback behavior.
- [x] Replace `workspace.rs` with `workspace/mod.rs` re-exports and delete the
  old flat module file.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Run `cargo fmt --all` in tui-kit.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in
  tui-kit.
- [x] Run `cargo test` in tui-kit.
- [x] Verify `git status --short -- visual-tests` in tui-kit still reports
  only the pre-existing untracked directory.
- [x] Commit the completed Phase 14 work.

## Verification Commands

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

tui-kit:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short -- visual-tests
```

## Commit Scope

One c4tui commit should include:

- the workspace module split;
- moved and focused workspace tests;
- any import adjustments needed by the new private module boundaries.

The tui-kit repo should receive only this plan document.
