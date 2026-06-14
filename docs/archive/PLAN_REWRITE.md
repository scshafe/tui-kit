# Plan: tui-kit to optimal design

The frame proposed earlier is good but conservative — it preserves what's there and re-establishes the feedback loop around it. The decision here is more aggressive: cut what hasn't earned its keep, restore the consumer feedback loop, and let any module that comes back come back through the loop.

## Target end state

A consumer-driven, policy-light middleware where:

1. **Every public module has at least one in-tree consumer** — c4tui, an example that actually runs, or a test that exercises real semantics (not just types).
2. **The API floor is explicit**: every public item carries stability metadata; `experimental` is a real, visible status, not "well, we're pre-1.0."
3. **Config types form one hierarchy** with consistent, non-doubled error paths and a single pattern (`Validate` trait + `ConfigError { path, reason }`) — no two-layer wrappers like `ImageConfig` over `ImageBackendPreference`.
4. **Test doubles share semantics with the real implementation** — `DeterministicScheduler` and `Scheduler` cancel, prioritize, and report identically; passing against one means passing against the other.
5. **A consumer gate runs in CI**: tui-kit changes that break c4tui fail the build at the tui-kit PR, not 24 commits later.
6. **`PLAN.md` and `README.md` describe the actual crate**, including which surfaces are experimental and what each module's consumer is.

## Current component-system follow-up

The new element/window/grid layer is now the active rollout area. Keep c4tui
out of the critical path until the library primitives are better flushed out;
return to c4tui after the local semantics below are tested and documented.

**Deferred note:** `WindowRepaintPolicy` is not urgent right now. The current
`Whole` and `ChildCached` tests are enough for the moment; deeper repaint-region
and cache-invalidation semantics should be revisited later as a focused pass.

**Now: harden the new primitives.**
- Add direct tests for `Window` lifecycle, focus-scope metadata, active key
  participation, resize hooks, and scoped image/effect teardown.
- Add direct tests for `Modal` activation/focus behavior and key routing.
- Add direct tests for `Overlay` render order, topmost routing, and modal
  capture.
- Add `Grid` edge tests for empty collections, one-column list behavior, fixed
  columns in narrow viewports, no-wrap boundary navigation, active-cell scroll
  anchoring, and clipped-cell scroll indicators.

**Next: put decorator functionality front and center.**
- Specify and test `scroll_y` as a real behavior decorator: scroll offset,
  clipping, dirty propagation, child rendering area, and key/event interaction.
- Specify and test `focusable`: stable focus IDs, enabled/visible state, child
  forwarding, and composition with `Window` focus scopes.
- Specify and test `with_keymap`: child-first precedence, local message
  emission, inactive/focused participation rules, and composition with windows
  and overlays.
- Specify and test `with_padding` and `with_border`: child-area math,
  rendering, dirty/layout invalidation, nested decorator order, and zero-sized
  area behavior.
- Effect forwarding belongs on geometry-safe single-child wrappers in v1:
  `Panel`, `Focusable`, `KeyMapped`, `Padded`, `Bordered`, and `Modal` forward
  child effects and teardown. `ScrollY` remains excluded until terminal effects
  have explicit clipping/source-cropping semantics.
- Multi-child effect composition is opt-in: `Stack` and `Overlay` keep their
  plain child APIs and add explicitly named effect-aware APIs
  (`Stack::push_effect_child`, `Stack::with_effect_child`,
  `Overlay::push_effect_layer`, `Overlay::with_effect_layer`,
  `Overlay::push_modal_effect_layer`, `Overlay::with_modal_effect_layer`) so
  render-only and effectful children can coexist without forcing every child
  into the effect contract.

**Polish backlog.**
- Explore fine-print text treatments for compact helper/status copy. Treat
  true small fonts as terminal-capability-dependent; provide a portable fallback
  using style, dimming, placement, truncation, and density rather than assuming
  terminals can render per-region font sizes.
- Explore icon integration for command affordances, status markers, log levels,
  and picker/list cells. Prefer explicit icon sets and fallback glyph policy
  over ad hoc Unicode literals.
- Expand typed effects beyond images when a real consumer needs them: cursor
  changes, clipboard actions, terminal title changes, attention/bell, transient
  visual effects, and scoped teardown.
- Explore `Window` as a logging boundary: local drains/ring buffers,
  lifecycle-driven attach/detach, focus-aware level elevation, per-window
  structured fields, modal diagnostic capture, and subscriptions to specific
  drains such as app, scheduler, image, input, picker, and workspace watcher.
  Keep implicit log context inspectable with a typed scope such as
  `WindowLogScope { window_id, fields, drain, min_level }`; avoid hidden global
  mutation. Prefer proving this as a `LogWindow` or `WindowLogLayer` consumer
  of base `Window` hooks before baking logging into `Window` itself.

## Phase 1 — Demolition

Delete every module that has no in-tree consumer right now.

**Delete entirely:**
- `src/widgets/list.rs`
- `src/widgets/table.rs`
- `src/widgets/tree.rs`
- `src/widgets/tabs.rs`
- `src/theme.rs`
- `src/runtime.rs`
- `src/subscription.rs`
- All prelude re-exports for the above
- All README rows for the above

**Collapse type-wrapper churn:**
- Inline `ImageConfig` back into `TerminalConfig`. One field, one set of presets.
- Inline `WatcherConfig` back to positional `WorkspaceWatcher::spawn(paths, debounce, sink)` unless c4tui needs multiple named watchers.
- Drop `WatcherSourceId` and `WatcherEvent::WorkspaceChanged { id }` — revert to the unit variant — unless c4tui actually needs to disambiguate.
- Drop `SchedulerConfig` if c4tui passes a worker count directly. Keep `Scheduler::try_new` returning `ConfigError` only if there's something to validate beyond `worker_count > 0`.

**Drop test-concern leaks from production types:**
- Remove `TickConfig.allow_subproduction_interval_for_tests`. Replace with a private constructor used only from `cfg(test)` or split test-fast into a sealed type.

**Drop dead test-double surface:**
- Until the deterministic scheduler models cancellation: either implement `RequestScope`-based cancellation in `DeterministicScheduler` (preferred) or drop the `scope` field so it can't lie.

## Phase 2 — Restore c4tui as the validation harness

c4tui compiles green against tui-kit, treating every error as design feedback.

Per-error rule: before fixing the c4tui side, look at the tui-kit side and ask "would I design this API this way if I were starting over with c4tui's needs in front of me?" If no, change tui-kit.

The 15 errors break into four design conversations:

1. **`AppEvent` shape** — keep categorized + `UserEvent`-generic shape, port c4tui's match arms. Simplify if migration reveals friction.
2. **`WorkspaceWatcher::spawn`** — revert to positional args unless c4tui actually needs named routing.
3. **`Scheduler::new`** — either keep `try_new` and have c4tui call it, or make `new` take a `NonZeroUsize`.
4. Other c4tui-internal errors (RenderedView, RasterBudget, render_svg) are c4tui's problem.

## Phase 3 — Earn back what was cut

Probationary Phase-3 core (`Component`/`Cached`/`Focus`/`Tick`) gets its loop closed:

- **`Component` + `Cached`**: port one c4tui widget — picker — to implement `BufferComponent`. If trait shapes don't fit, change them.
- **`Focus`**: rewire c4tui's mode-based event routing through `FocusManager`.
- **`Tick`**: replace c4tui's `RuntimeEvent::Heartbeat` with a `TickConfig` source.

Cut modules come back only when:
- **Theme** — c4tui converts at least picker's selection/border styles to lookup-by-`ThemeRole`.
- **A second widget** — only when c4tui or a real second app needs it.
- **Subscriptions** — only when c4tui's workspace watcher emits `UpdateEvent`s through the unified channel and a component reads them.
- **Runtime config** — only when there's a consumer that wants a single bundle.

## Phase 4 — Correctness and diagnostic-surface pass

- **Path-prefixing bug** (`runtime.scheduler.scheduler.worker_count`) — obsolete after runtime/scheduler config demolition; keep future config paths contract-first.
- **Audit every test that asserts a `ConfigError.path`**: rewrite from "what the code produces" to "what the docs/contract require."
- **`DeterministicScheduler` cancellation parity**: scope/id/group/source/epoch queued-cancellation parity now covered against the real scheduler; add property tests before expanding the double further.
- **`Cached.stats()`**: done as `cache_hits` / `cache_misses`.
- **`watcher::is_relevant`**: done; watched paths are classified once at spawn time.
- **`WatcherSourceId::new` vs `Validate`** — obsolete after watcher config/source-id demolition.
- **README and PLAN.md**: keep aligned with the current crate, not the historical roadmap.

## Phase 5 — Reinforcement

- **Stability annotations** on every `pub` item at module level.
- **Consumer gate in CI**: cargo fmt/clippy/test/doc plus c4tui `cargo check` against local tui-kit.
- **Single-PR rule for breaking changes**.
- **`pub use` discipline**: prelude exports only production consumer surface; testkit helpers are no longer re-exported from the prelude.
- **Property-test the test harness**: `tests/parity.rs` submits same workload to `Scheduler` and `DeterministicScheduler`.

## Phase 6 — Lock and version

- Cut `0.2.0` on crates.io that reflects the post-demolition surface.
- Tag stable API surface as `pub`, everything not yet earned as `pub(crate)`.
- Add `examples/` that includes at least one example that actually runs a terminal.

## Sequencing

1. Phase 1 + relevant Phase 4 (delete-the-dead-code bugs) — one sweeping change. Crate breaks; that's fine.
2. Phase 2 — c4tui restored. One PR per design conversation, each landing in tui-kit + c4tui together.
3. Phase 3 — re-add what gets re-asked-for, each gated by an actual c4tui change.
4. Phase 4 remainder — correctness/diagnostic debt.
5. Phase 5 — CI gate + stability annotations + parity tests.
6. Phase 6 — version cut.

Total LOC deleted in Phase 1 is roughly 4500-5500. Most will not come back. The crate at the end of Phase 6 is materially smaller, materially more validated, and meaningfully harder to drift.

## What this plan deliberately rejects

- **Compatibility shims** to keep c4tui compiling against current tui-kit.
- **"Mark as experimental" instead of delete** for Phase 1.
- **Keep all four widgets because they're already tested**.
- **Defer the CI gate until later**.

The bet: small-and-validated beats large-and-speculative.
