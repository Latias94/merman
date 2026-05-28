# Resvg-Safe SVG Output Pipeline - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

Focused gates for early slices:

```bash
cargo nextest run -p merman-render foreign_object_overlay_splits_literal_backslash_n
cargo nextest run -p merman-render svg::parity::fallback::tests::foreign_object_overlay
```

Pipeline/API gates:

```bash
cargo nextest run -p merman-render svg
cargo nextest run -p merman
cargo nextest run -p merman-cli png_smoke jpeg_smoke pdf_smoke
cargo fmt -p merman-render -p merman -- --check
cargo clippy -p merman-render -p merman --all-targets -- -D warnings
git diff --check
```

## Evidence Anchors

- `docs/adr/0063-extensible-svg-output-pipeline.md`
- `crates/merman-render/src/svg/parity/fallback.rs`
- `crates/merman/src/lib.rs`
- `crates/merman/src/render/raster.rs`
- `repo-ref/zed/crates/mermaid_render`

## Fresh Evidence

2026-05-28:

- `rustfmt crates/merman-render/src/svg/parity/fallback.rs` - PASS.
- `cargo nextest run -p merman-render foreign_object_overlay_splits_literal_backslash_n`
  - PASS, 1 test passed.
- `cargo nextest run -p merman-render svg::parity::fallback::tests::foreign_object_overlay`
  - PASS, 3 tests passed.
- `cargo fmt -p merman-render -- --check` - PASS after formatting.
- `git diff --check` - PASS.

2026-05-28 closeout:

- `cargo nextest run -p merman-render svg::pipeline`
  - PASS, 5 tests passed.
- `cargo nextest run -p merman --features render svg_pipeline_tests`
  - PASS, 2 tests passed.
- `cargo nextest run -p merman-render fallback`
  - PASS, 6 tests passed.
- `cargo nextest run -p merman-render svg`
  - PASS, 73 tests passed.
- `cargo nextest run -p merman-render`
  - PASS, 213 tests passed.
- `cargo nextest run -p merman --no-tests=pass`
  - PASS, 0 tests run because default-feature `merman` currently has no tests.
- `cargo nextest run -p merman --features raster`
  - PASS, 14 tests passed.
- `cargo nextest run -p merman-cli`
  - PASS, 8 tests passed.
- `cargo fmt -p merman-render -p merman -p merman-cli -- --check`
  - PASS.
- `cargo clippy -p merman-render -p merman --all-targets -- -D warnings`
  - PASS after adding the missing `required-features = ["render"]` gate to
    `architecture_stress`.
- `cargo clippy -p merman --features raster --all-targets -- -D warnings`
  - PASS.
- `cargo clippy -p merman-cli --all-targets -- -D warnings`
  - PASS.
- `git diff --check`
  - PASS.

## Notes

- Zed's `crates/mermaid_render` wrapper is GPL-licensed. Treat it as integration evidence only.
- The default `render_svg_sync` parity path must stay unchanged unless a task explicitly says
  otherwise and parity gates are rerun.
- Default SVG output was intentionally left unoptimized. Consumer cleanup is explicit through
  `SvgPipeline`, while raster/readable paths opt in to `Readable` or `ResvgSafe`.
