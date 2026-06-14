# Phase 11 - c4tui App Orchestration and Modal Factory

**Status:** COMPLETE - implemented and verified, 2026-06-14  
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`  
**Roadmap source:** `2026-05-12-tui-kit-c4tui-refactor-roadmap.md` and Phase 10 follow-on list.  
**Predecessor:** `2026-06-13-phase-10-code-quality-assessment.md`

## Goal

Extract modal construction and post-command effect execution from
`c4tui/src/app.rs` without changing user-visible behavior. `App` remains the
event-loop owner, but view/child-view/link modal construction and effect
dispatch move behind focused modules that can be tested directly.

This phase does not start Phase 12. It must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

Phase 3 through Phase 6 concepts are already present:

- `c4tui/src/modal.rs` defines `ActiveModal`, `NavPickerModal`, `Modal`, and
  modal outcome traits.
- `c4tui/src/nav_picker.rs` provides `NavPicker<T>`.
- `c4tui/src/nav_items.rs` provides `ViewNavItem` and `NavTarget`.
- `c4tui/src/link_directory.rs` provides `LinkCandidate` and
  `collect_link_candidates`.
- `c4tui/src/state.rs` emits `Effect::OpenModal(ModalSpec)`,
  `Effect::ReloadWorkspace`, `Effect::ClearImageCache`, `Effect::ShowHelp`,
  `Effect::ToggleLogView`, and `Effect::Quit`.

The remaining problem is that `App::handle_input` still owns:

- modal construction for all `ModalSpec` variants;
- focus-scope push for picker and link-directory scopes;
- initial hover render priming for view, child-view, and link modals;
- effect execution for reload, image-cache clearing, help/log dialogs, quit,
  and render policy.

## File Structure

Create:

- `/Users/coleshaffer/Projects/c4tui/src/modal_factory.rs`
- `/Users/coleshaffer/Projects/c4tui/src/effect_runner.rs`

Modify:

- `/Users/coleshaffer/Projects/c4tui/src/main.rs`
- `/Users/coleshaffer/Projects/c4tui/src/app.rs`
- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-06-14-phase-11-c4tui-app-orchestration-modal-factory.md`

No tui-kit public API changes are planned.

## Design

### Modal Factory

Add `modal_factory` as the single construction point for
`Effect::OpenModal(ModalSpec)`.

Planned API:

```rust
pub struct ModalSpawn {
    pub scope_id: &'static str,
    pub focus_node_id: &'static str,
    pub active_modal: ActiveModal,
    pub initial_hover: Option<ViewId>,
}

pub fn spawn_modal(
    spec: ModalSpec,
    catalog: &ViewCatalog,
    state: &AppState,
) -> ModalSpawn
```

Responsibilities:

- create the correct `NavPicker` and wrap it in `NavPickerModal`;
- compute the correct initial selection;
- attach the correct `NavTarget -> Command` callback;
- report focus scope/node ids for the caller to push;
- report the initial hover view, if any.

Non-responsibilities:

- no terminal calls;
- no focus mutation;
- no scheduler mutation;
- no active-modal assignment;
- no rendering.

`App` will continue to own the actual focus manager, active modal slot,
terminal teardown, scheduler, and render cache. This keeps the factory pure
enough for direct tests.

### Effect Runner

Add `effect_runner` to isolate effect dispatch and render policy from
`App::handle_input`.

Planned API:

```rust
pub struct EffectRunner<'a, B: TerminalBackend> {
    pub terminal: &'a mut B,
    pub catalog: &'a mut ViewCatalog,
    pub render_cache: &'a mut RenderCache,
    pub viewport: &'a mut ViewportState,
    pub state: &'a mut AppState,
    pub scheduler: &'a mut RenderScheduler,
    pub focus: &'a mut FocusManager,
    pub active_modal: &'a mut Option<ActiveModal>,
    pub dialog_slot: &'a mut Option<DialogSlot>,
    pub config: &'a AppConfig,
    pub keymap: &'a mut KeyMap,
    pub workspace: &'a WorkspaceSource,
    pub svg_format: &'a str,
    pub quit: &'a mut bool,
}
```

This shape may be narrowed while implementing. The final context should expose
only the fields the executor really needs.

Responsibilities:

- execute `Effect::Quit`;
- open modal specs through `modal_factory`;
- run reload and reload-failure dialog handling;
- clear image cache;
- show help and enter a dialog scope;
- toggle log view through the existing log modal path;
- centralize root/modal redraw decisions after command application.

Non-responsibilities:

- no event receive loop;
- no input translation;
- no modal key handling;
- no new command/effect variants.

## Tasks

- [x] Confirm Phase 11 scope from the roadmap and Phase 10 assessment.
- [x] Confirm both working trees are clean except the pre-existing
  `tui-kit/visual-tests/` directory.
- [x] Map current `App::handle_input`, `handle_modal_key`, modal types, and
  app tests.
- [x] Add this Phase 11 plan document.
- [x] Add `modal_factory` with direct tests for `ModalSpec::View`,
  `ModalSpec::ChildView`, and `ModalSpec::Link`.
- [x] Replace inline `Effect::OpenModal` construction in `App` with
  `modal_factory::spawn_modal`.
- [x] Add `effect_runner` and move effect dispatch/render policy out of
  `App::handle_input`.
- [x] Keep modal-key handling behavior unchanged and update tests only where
  call ordering changes because helper boundaries become explicit.
- [x] Run `cargo fmt --all` in c4tui.
- [x] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [x] Run `cargo test` in c4tui.
- [x] Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`,
  and `cargo test` in tui-kit.
- [x] Confirm tui-kit public surface did not change; if it did, run the
  public-surface/docs audit checks before committing.
- [x] Commit the completed Phase 11 work.

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

If tui-kit public Rust API changes unexpectedly, also run the documented
retention/public-surface audit commands before the commit.

## Commit Scope

One commit should include:

- this Phase 11 plan document;
- c4tui modal factory extraction;
- c4tui effect runner extraction;
- test updates and any new focused tests.

Do not include unrelated cleanup, Phase 12 work, or `visual-tests/` changes.
