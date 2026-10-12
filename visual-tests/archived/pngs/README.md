# Archived image geometry fixtures

These nine PNGs preserve the original image-box source fixture and eight expected crop cases from the retired visual-test harness. They are historical reference data; no current automated test executes them.

- `image_box_source_fixture.png`: 800 × 600 grid, with a 200 × 150 origin rectangle and a 100 × 75 rectangle at (460, 300).
- `expected/`: expected images named by widget dimensions, scaled-image offset and scalar.

The fixture history and coordinate conventions are documented in `docs/archive/CODEX_SESSION_019e1141_SUMMARY.md`. Regenerable comparison images, compilation outputs and Python caches are excluded from this preservation commit.
