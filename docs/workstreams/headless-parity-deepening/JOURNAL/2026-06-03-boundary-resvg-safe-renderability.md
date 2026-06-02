# HPD-080 Boundary Resvg-Safe Renderability

Date: 2026-06-03
Task: HPD-080
Status: done slice

## Context

The all-supported `resvg_safe` fixture audit intentionally tracks the implemented Mermaid-style
matrix in `SUPPORTED_FIXTURE_DIRS`. It does not include `info`, `error`, or `zenuml`:

- `info` has no dedicated Mermaid 11.15 style provider and uses the shared info-like base renderer.
- `error` is a host/suppress-errors entrypoint in addition to the literal `error` diagram.
- `zenuml` is a headless Sequence-compatibility subset, not full Mermaid browser plugin parity.

That boundary is correct, but it left these public/renderable entrypoints without the same
`resvg_safe` renderability smoke applied to the main supported families.

## Change

Added `boundary_fixtures_render_headless_resvg_safe` in
`crates/merman/tests/resvg_safe_fixture_smoke.rs`.

The new test:

- scans every `.mmd` fixture in `fixtures/error`, `fixtures/info`, and `fixtures/zenuml`,
- renders them through `HeadlessRenderer::render_svg_resvg_safe_sync(...)`,
- uses lenient parsing for the `fixtures/error` corpus so suppressed parse-error samples exercise
  the real host-visible error diagram path,
- keeps these fixtures separate from `SUPPORTED_FIXTURE_DIRS` so they do not imply full
  supported-family style-provider parity,
- reuses the same XML, `foreignObject`, invalid-token, empty-style, and optional PNG raster ink
  assertions as the main resvg-safe smoke.

For suppressed error fixtures, the ink check uses `error\n` as the visible source sentinel because
the input source may be parser-only while the generated error diagram must still be visible.

## Verification

- `cargo fmt -p merman`
- `cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke boundary_fixtures_render_headless_resvg_safe`
- `cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`

## Result

No production renderer defect was found in this slice. The value is a regression gate for public
boundary entrypoints that were not represented by the all-supported family matrix.

This should not be read as a broader ZenUML parity claim. Local ZenUML remains the documented
headless Sequence-compatibility subset.
