# Evidence And Gates

Workstream: Host Styling SVG Postprocessors
Last updated: 2026-05-28

## Planned Gates

- `cargo fmt -p merman-render -p merman -p merman-cli -- --check`
- `cargo nextest run -p merman-render`
- `cargo nextest run -p merman --features raster`
- `cargo nextest run -p merman-cli`
- `cargo clippy -p merman-render -p merman --features raster --all-targets -- -D warnings`
- `cargo clippy -p merman-cli --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-28: Workstream and ADR created. Gates pending implementation.
- 2026-05-28: Focused implementation gates:
  - `cargo nextest run -p merman-render svg::pipeline` passed: 11 tests.
  - `cargo nextest run -p merman-render foreign_object_overlay_propagates_style_context` passed:
    1 test.
  - `cargo check -p merman --features render --example svg_pipeline` passed.
  - `cargo nextest run -p merman --features render render_svg_with_pipeline_passes_parsed_metadata`
    passed: 1 test.
  - `Get-Content fixtures\flowchart\basic.mmd | cargo run -q -p merman --features render --example svg_pipeline`
    passed and emitted scoped CSS plus metadata.
- 2026-05-28: Full closeout gates:
  - `cargo nextest run -p merman-render` passed: 220 tests.
  - `cargo nextest run -p merman --features raster` passed: 15 tests.
  - `cargo nextest run -p merman-cli` passed: 8 tests.
  - `cargo clippy -p merman-render -p merman --features raster --all-targets -- -D warnings`
    passed.
  - `cargo clippy -p merman-cli --all-targets -- -D warnings` passed.
  - `cargo fmt -p merman-render -p merman -p merman-cli -- --check` passed.
  - `git diff --check` passed.
