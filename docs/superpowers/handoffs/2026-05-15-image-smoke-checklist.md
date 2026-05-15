# Operator Smoke-Test Checklist — tui-kit Image Path

**Status:** SCAFFOLD · 2026-05-15. Operator sign-off pending.
**Owner:** operator (live-terminal verification). Agents write this file;
operators run it.
**Plan reference:** Phase D of
[`../plans/2026-05-14-revised-library-author-implementation-plan.md`](../plans/2026-05-14-revised-library-author-implementation-plan.md)
and [`../plans/2026-05-15-phase-D-image-reliability.md`](../plans/2026-05-15-phase-D-image-reliability.md).

## What this checklist verifies

The library-side data flow for image upload, placement, and teardown is
locked by tests (see `src/image.rs` lifecycle scenarios). What tests
cannot verify is whether real terminals — locally and across SSH /
container layers — accept the Kitty graphics escape sequences tui-kit
emits and render the corresponding images.

This file is the operator's runbook for that verification. Sign-off lands
when every section is run and results are recorded in the Sign-off block
at the bottom.

## Test vehicle

tui-kit has no image example today (`examples/terminal_dialog.rs` runs in
`degraded_no_images` mode). The image path is consumed by **c4tui**, and
all sections of this checklist drive c4tui:

```bash
cd /Users/coleshaffer/Projects/c4tui
cargo run --release
```

If c4tui's `main` cannot be started in a given environment, that is the
finding for that section. Note the failure under "Failure modes".

## Common setup (run once per workstation)

Required toolchain:

- `cargo` (rustup-managed, stable channel).
- A Kitty-compatible terminal locally: Kitty ≥ 0.32 or WezTerm with Kitty
  graphics enabled.
- Either a real SSH target or a local OpenSSH server (`ssh localhost`).
- Docker or equivalent OCI runtime for the container sections.

Workstation preflight:

```bash
cd /Users/coleshaffer/Projects/tui-kit
cargo test --quiet 2>&1 | grep "test result:"   # 168 lib + 11 parity expected
cargo build --release -p c4tui --manifest-path /Users/coleshaffer/Projects/c4tui/Cargo.toml
```

Record terminal versions before starting:

| Terminal | Version | Notes |
|----------|---------|-------|
| Kitty    |         |       |
| WezTerm  |         |       |

## Section 1 — Local Kitty / WezTerm

### Setup

- Launch the target terminal natively (not via tmux/screen — multiplexers
  generally do not forward Kitty graphics).
- Window size at least 120×40 cells.

### Run

```bash
cd /Users/coleshaffer/Projects/c4tui
cargo run --release
```

### Expected

- c4tui enters alternate screen, raw mode, mouse capture, cursor hidden.
- The main view renders an image inline at expected cell coordinates.
- Navigating to the picker (per c4tui's keymap) shows thumbnail images.
- On exit (`q` or whatever c4tui binds), the alternate screen is left,
  the cursor is visible, the main shell prompt is restored, and no
  literal escape sequences are visible in the terminal scrollback.

### Failure modes

| Symptom | Likely cause |
|---------|--------------|
| Literal `\x1b_Ga=...` text in the terminal | Terminal does not understand Kitty graphics APC escapes. |
| Image area is blank but rest of UI renders | Kitty protocol acknowledged the upload but rejected the placement; check terminal logs. |
| UI never enters alt screen | Terminal does not honor crossterm's `EnterAlternateScreen`. |
| Mouse capture stuck after exit | `Drop` did not run; check for panic upstream. |

### Results

- Kitty:   _PASS / FAIL / NOTES:_
- WezTerm: _PASS / FAIL / NOTES:_

## Section 2 — SSH (terminfo + TERM propagation)

### Setup

- Identify an SSH target with `cargo` installed and the c4tui repo
  cloned. If no remote target is available, use `ssh localhost`.
- Confirm `TERM` and terminfo on the remote:
  ```bash
  ssh -t target 'echo "TERM=$TERM"; infocmp -1 "$TERM" 2>&1 | head -5'
  ```
- Expected: `TERM` is something like `xterm-kitty`, `xterm-256color`, or
  `wezterm`; terminfo lookup succeeds.

### Run

```bash
ssh -t target 'cd ~/c4tui && cargo run --release'
```

Repeat from both Kitty and WezTerm locally.

### Expected

- Image escapes flow through the SSH PTY to the local terminal.
- Same rendering behavior as Section 1.

### Failure modes

| Symptom | Likely cause |
|---------|--------------|
| Images do not render but UI is otherwise correct | SSH PTY layer dropped or filtered the APC escapes. |
| `TERM` arrives as `dumb` or empty | `AcceptEnv`/`SendEnv` mis-configured on the SSH endpoint; tui-kit does not depend on `TERM` itself, but downstream consumers may. |
| Slow image transmission | Single-threaded `transmit_png` chunks at 4096 bytes; SSH path adds round-trip overhead. Expected, not a bug. |

### Results

- ssh + Kitty:   _PASS / FAIL / NOTES:_
- ssh + WezTerm: _PASS / FAIL / NOTES:_

## Section 3 — SSH into container (PTY + Kitty passthrough)

### Setup

- Container image with `cargo` and the c4tui source available. Bind-mount
  the repo or pre-bake it.
- SSH into the host first, then `docker exec -it` into the container:
  ```bash
  ssh -t host 'docker exec -it <container> bash -lc "echo TERM=$TERM; tty"'
  ```
- Confirm a PTY is allocated (`tty` returns a `/dev/pts/...` device).

### Run

```bash
ssh -t host 'docker exec -it <container> bash -lc "cd /work/c4tui && cargo run --release"'
```

### Expected

- Same as Section 2, but with the additional `docker exec` PTY layer.
- Images render in the local terminal.

### Failure modes

| Symptom | Likely cause |
|---------|--------------|
| `docker exec` aborts with "the input device is not a TTY" | Missing `-t` somewhere in the chain. |
| Images do not render at all | Either the container terminfo is missing the entry for the inherited `TERM`, or the `docker exec` PTY filters APC escapes. |
| Garbage cells where image should be | Cursor-positioning escapes for `place_at` survived but image escapes did not. |

### Results

- ssh→docker exec + Kitty:   _PASS / FAIL / NOTES:_
- ssh→docker exec + WezTerm: _PASS / FAIL / NOTES:_

## Section 4 — Local docker exec (no SSH)

### Setup

- Same container as Section 3, but executed directly on the host without
  SSH.

### Run

```bash
docker exec -it <container> bash -lc "cd /work/c4tui && cargo run --release"
```

### Expected

- Removes the SSH layer as a variable. Useful for narrowing whether a
  Section 3 failure is in the SSH layer or the docker layer.

### Results

- Kitty:   _PASS / FAIL / NOTES:_
- WezTerm: _PASS / FAIL / NOTES:_

## Known passthrough edge cases

This list is pre-populated from code reading; operators should extend it
with observed cases.

- **No write-side lock on `transmit_png`.** `transmit_png` writes 4096-byte
  base64 chunks to `io::stdout().lock()` per chunk (`src/image.rs:476-498`),
  releasing the lock between chunks. tui-kit does not run concurrent
  `transmit_png` calls today, but if a future consumer does, PTY-layer
  buffering could interleave chunks from different `image_id`s. Currently
  serialized by the single-threaded render path.
- **No protocol probing.** tui-kit does not query the terminal for Kitty
  graphics support. The `ImageBackendPreference::KittyOnly` preset assumes
  the operator has chosen a compatible terminal. Pass-through environments
  that silently drop APC escapes will be silent failures, not crashes.
- **Cursor-positioning escapes are unconditional.** `place_at` issues a
  `\x1b[r;cH` cursor move before each placement. If a PTY layer filters
  cursor escapes (rare), images mis-place but no error is observable
  in-band.
- **`tmux` and `screen` typically strip Kitty graphics.** Run the
  checklist outside multiplexers unless a specific multiplexer + terminal
  combination is being verified.

### Operator additions

_Add observed edge cases here, with the section that surfaced them._

- (none yet)

## Sign-off

When every section above is run and results recorded, fill in the block
below and commit this file. Per the parent plan, Phase D Exit criterion
#3 is gated on this sign-off.

| Field | Value |
|-------|-------|
| Date  |       |
| Operator |   |
| tui-kit HEAD | (run `git -C /Users/coleshaffer/Projects/tui-kit rev-parse HEAD`) |
| c4tui HEAD   | (run `git -C /Users/coleshaffer/Projects/c4tui rev-parse HEAD`) |
| Terminals tested | |
| Pass / Fail summary | |
| Outstanding issues | |

Once signed, link this file from `docs/superpowers/handoffs/` (it
already lives there) and update the Phase D parent-plan status to mark
exit criterion #3 closed.
