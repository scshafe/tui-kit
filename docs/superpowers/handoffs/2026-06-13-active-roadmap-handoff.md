# Active Handoff - Phase 7 Through Phase 10

## Current State

Phases 1-6 of the tui-kit + c4tui refactor sequence are complete and committed.
Completed or stale phase plans and handoffs have been archived under
`docs/superpowers/archive/` in each repo. Older root-level planning artifacts
have been archived under `docs/archive/`.

Active forward roadmap:

- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/plans/2026-05-12-tui-kit-c4tui-refactor-roadmap.md`

Active operator-owned checklist:

- `/Users/coleshaffer/Projects/tui-kit/docs/superpowers/handoffs/2026-05-15-image-smoke-checklist.md`

## Required Workflow

Implement Phase 7, Phase 8, and Phase 9 one after another as a single
comprehensive workflow:

1. Write the detailed Phase 7 plan, implement it, verify both repos, commit.
2. Immediately write the detailed Phase 8 plan, implement it, verify, commit.
3. Immediately write the detailed Phase 9 plan, implement it, verify, commit.

Do not pause after Phase 7 or Phase 8 just to ask what comes next. Pause only
for a real blocker, an external dependency, or a user instruction that changes
the sequence.

## Upcoming Phases

### Phase 7 - Documentation + Retention Tooling

Scope:

- Add a tui-kit public-surface retention audit.
- Reconcile README/specification/architecture docs in both repos against the
  post-Phase-6 code.
- Archive or clearly mark historical c4tui implementation planning.
- Add the c4tui SVG -> PNG -> Kitty registry -> placement pipeline diagram.
- Write cross-repo testing-pattern docs for tui-kit `testkit` and c4tui
  `FakeTerminalBackend`.

### Phase 8 - RenderEffect + Renderer Backend Model

Scope:

- Make tui-kit's render/effect contract explicit and renderer-neutral enough
  for future remote rendering.
- Keep local terminal rendering as the working adapter.
- Add an in-process mock renderer boundary.
- Tighten `elements` only where it directly supports this renderer contract.

### Phase 9 - tui-kit-cli Remote Launcher Prototype

Scope:

- Add a prototype client launcher that can run a compatible remote tui-kit app
  over SSH/PTTY transport.
- Forward typed input, mouse, and resize events.
- Render remote buffer/effect frames locally using the Phase 8 backend.
- Keep configuration minimal until the basic run path works.

### Phase 10 - Intensive Code Quality Assessment

Phase 10 is the next-session review phase after Phase 7-9 land. It should not
perform broad refactors. It should produce a committed assessment document with
findings, code references, remediation waves, risks, and test strategy.

### Phase 11+

Phase 11 and later should be generated from the Phase 10 findings and should
act on those findings in small, verifiable refactor waves.

## Guardrails

- No backwards-compatibility shims, deprecation layers, or old-name aliases.
- Every public tui-kit surface needs a real consumer or a documented reason to
  remain.
- Keep docs factual. Do not write aspirational docs as if future phases already
  exist.
- Each implementation phase ends with fmt, clippy, tests, and a commit.
- Leave `/Users/coleshaffer/Projects/tui-kit/visual-tests/` untouched unless
  the user explicitly asks for cleanup.

## Known Open Item

The image smoke checklist still requires operator sign-off in live local,
SSH, and container terminal environments. Agents may update the checklist
format, but only the operator can complete the sign-off.
