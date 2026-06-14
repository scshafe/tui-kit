# Phase 18 - c4tui Runtime Transaction Boundary

**Status:** PLANNED - do not implement until explicitly requested, 2026-06-14
**Repos:** `/Users/coleshaffer/Projects/tui-kit`, `/Users/coleshaffer/Projects/c4tui`
**Roadmap source:** `2026-06-14-post-phase-17-architecture-followup-handoff.md`
**Predecessors:** Phase 17 public API and test fixture hygiene.

## Goal

Introduce a single c4tui runtime transaction boundary so root commands, modal
keys, config cycling, reloads, render scheduling, and terminal side effects all
flow through one executor. `App` should remain the event-loop owner, but it
should stop owning branch-heavy render policy and duplicated reload/reset
logic.

This phase must preserve current user-visible behavior. It must not start Phase
19 terminal/backend work, and it must not touch
`/Users/coleshaffer/Projects/tui-kit/visual-tests/`.

## Current Shape

After Phase 17:

- `c4tui/src/app.rs` owns the main event loop, event coalescing, input routing,
  modal-key transitions, config cycling, watcher reloads, root rendering, modal
  rendering, and render-cache priming.
- `c4tui/src/effect_runner.rs` owns post-command effect execution for root
  commands, including modal opening, manual reload, cache clear, log/help
  dialogs, quit, active-render requests, and render policy.
- Manual reload reset logic exists in `EffectRunner::reload_store`; watcher
  reload reset logic exists separately in `App::reload_store`.
- `App::handle_input` short-circuits `Command::CycleScaleBasis`,
  `Command::CycleOverflow`, and `Command::CycleZoomStep` before the
  command/effect path.
- `App::handle_modal_key` directly decides hover render prefetch, modal
  re-render, modal close, command application after selection, focus-scope
  cleanup, active-render requests, and root render.
- `redraw_for_mode`, `frame_with_progress`, and `request_active_render` exist
  in both `App` and `EffectRunner`.

## Target Boundary

Add a new internal module:

- `/Users/coleshaffer/Projects/c4tui/src/runtime.rs`

Use the names below as the implementation target:

- `AppRuntime<'a, B>`: a short-lived executor built from mutable references to
  the current app state and a `TerminalBackend`.
- `RuntimeIntent`: an internal data enum for requested runtime work.
- `RuntimeTransaction`: an internal accumulator that records render,
  scheduler, modal, dialog, reload, cache, and quit intents before execution.
- `ReloadOrigin`: an internal enum for `Manual` and `Watcher` reload message
  differences.

`App` should keep:

- owned fields;
- constructor setup;
- event receive loop;
- resize coalescing;
- conversion from incoming `AppEvent` to a runtime call;
- final `quit` break condition.

`AppRuntime` should own:

- root command transactions;
- config cycling transactions;
- modal key transactions;
- manual and watcher reload execution;
- active/hover render request scheduling;
- focus/modal/dialog transitions;
- root/modal/dialog render decisions;
- image-cache clear decisions;
- frame-with-progress construction.

## Design

### Runtime Transaction

`RuntimeTransaction` should be explicit enough that tests can assert the
decision before terminal calls are performed.

Planned intent categories:

```rust
pub(crate) enum RuntimeIntent {
    RequestActiveRender,
    RequestHoverRender(ViewId),
    RenderRoot,
    RenderModal,
    CloseModal,
    TeardownRootViewport(ViewId),
    ClearImageCache,
    ShowHelp,
    ShowMessage { title: &'static str, message: &'static str },
    ShowError { title: &'static str, message: String },
    EnterDialog { dismissable: bool },
    ReturnToRoot,
    Quit,
}
```

The final enum can be narrower if implementation proves that some intents are
only private helper calls. The important rule is that command/modal handling
should record intent first, and one executor should decide terminal ordering.

### Command Flow

`AppRuntime::handle_root_input(input)` should:

- read current `CanvasMetrics` once from the backend;
- resolve the input through the current keymap;
- handle config commands through the same transaction executor as state
  commands;
- call `AppState::apply` for normal commands;
- translate `UpdateResult` and `Effect` into `RuntimeIntent` values;
- execute the resulting transaction.

Config commands should no longer be special render branches in `App`. Preserve
current behavior:

- `CycleScaleBasis`: cycle config placement scale basis, recompute placement
  policy, clear derived viewport placement state, request active render, render
  root.
- `CycleOverflow`: same as above for overflow.
- `CycleZoomStep`: cycle zoom config, rebuild keymap, request active render,
  render root.

Do not add new compatibility commands or aliases.

### Modal Key Flow

`AppRuntime::handle_modal_key(key)` should replace the decision body currently
inside `App::handle_modal_key`.

Preserve current behavior:

- picker/log continue re-renders the active modal only;
- hovered picker view requests a hover-priority render if it is not cached;
- nav selection closes modal placements, pops focus, applies the selected
  command, requests active render, then renders root;
- nav cancel closes modal placements, pops focus, and renders root;
- log close returns to root and renders root;
- modal scopes still absorb mouse input unless the active scope is root.

### Reload Flow

Create one reload implementation that both manual reload and watcher reload
call.

Planned API:

```rust
pub(crate) enum ReloadOrigin {
    Manual,
    Watcher,
}

impl ReloadOrigin {
    fn start_message(self) -> (&'static str, &'static str);
    fn failure_title(self) -> &'static str;
    fn failure_log_prefix(self) -> &'static str;
}
```

Preserve current message behavior unless an implementation finding is brought
back for approval:

- watcher start message remains `Workspace changed` /
  `Re-running Structurizr export.`;
- manual start message remains `Reloading workspace...` /
  `Re-running Structurizr export.`;
- watcher failure dialog title remains `Auto-reload failed`;
- manual failure dialog title remains `Reload failed`;
- successful reload still resets navigation through `Command::ReloadSucceeded`;
- failed reload still applies `Command::ReloadFailed`;
- successful reload still clears image cache when the state update returns
  `Effect::ClearImageCache`;
- watcher reload still returns to root before redraw.

Move the duplicated store reset into a single helper that rebuilds:

- `ViewCatalog`;
- `RenderCache`;
- `ViewportState`;
- scheduler invalidation and background render requests.

### Render Policy

`AppRuntime` should own a single `redraw_for_mode` equivalent. After Phase 18
there should be no duplicate `frame_with_progress`, `request_active_render`, or
reload-store helper in both `App` and the effect/runtime executor.

Render policy must stay behavior-compatible:

- root scope renders root;
- picker/link/log scopes render modal;
- dialog scope does not redraw root on ordinary updates;
- scheduler completions redraw according to current active scope;
- startup still renders root once and requests the active render.

## Non-Goals

- No terminal rendering/backend split; that is Phase 19.
- No image geometry canonicalization; that is Phase 20.
- No visibility cleanup beyond what is needed for the new module; that is Phase
  21.
- No tui-kit public API changes.
- No behavior changes to reload messages, terminal call ordering, key handling,
  modal focus, or render scheduling unless explicitly called out for approval.

## Implementation Tasks

- [ ] Confirm both working trees are clean except the pre-existing untracked
  `tui-kit/visual-tests/` directory.
- [ ] Add this Phase 18 plan document if it is not already present in the
  implementation branch.
- [ ] Add `mod runtime;` and create `c4tui/src/runtime.rs`.
- [ ] Move shared helpers from `effect_runner.rs` and `app.rs` into
  `runtime.rs`: active scope lookup, frame-with-progress construction,
  active/hover render requests, mode-aware redraw, return-to-root, and dialog
  entry.
- [ ] Introduce `RuntimeTransaction` and `RuntimeIntent` with direct unit tests
  for root render, modal render, close-modal-plus-root-render, dialog entry,
  cache clear, and quit decisions.
- [ ] Replace `EffectRunner` with `AppRuntime` or convert `EffectRunner` into a
  thin compatibility-free runtime executor. Do not leave both executors owning
  separate behavior.
- [ ] Route root input handling through `AppRuntime::handle_root_input`.
- [ ] Route config cycling through runtime transactions instead of
  short-circuiting in `App::handle_input`.
- [ ] Route modal key handling through `AppRuntime::handle_modal_key`.
- [ ] Unify manual and watcher reload store reset behind one helper and one
  `ReloadOrigin`-aware reload transaction.
- [ ] Remove duplicate `reload_store`, `redraw_for_mode`,
  `frame_with_progress`, and `request_active_render` implementations.
- [ ] Update existing app/effect-runner tests to target runtime transactions
  and preserve existing `FakeTerminalCall` ordering.
- [ ] Add targeted tests for watcher reload success, watcher reload failure,
  manual reload success, manual reload failure, and config cycling render
  policy if existing coverage is not already precise enough.
- [ ] Run `cargo fmt --all` in c4tui.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` in c4tui.
- [ ] Run `cargo test` in c4tui.
- [ ] Run tui-kit verification only if the implementation unexpectedly touches
  tui-kit or dependency behavior.
- [ ] Commit the completed Phase 18 work before starting Phase 19.

## Verification Commands

c4tui:

```sh
cd /Users/coleshaffer/Projects/c4tui
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

tui-kit, only if touched:

```sh
cd /Users/coleshaffer/Projects/tui-kit
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short -- visual-tests
```

## Commit Scope

One c4tui commit should include:

- runtime transaction boundary extraction;
- duplicate reload/render helper removal;
- focused runtime/app tests.

tui-kit should have no source changes in this phase unless verification exposes
a real dependency issue.

