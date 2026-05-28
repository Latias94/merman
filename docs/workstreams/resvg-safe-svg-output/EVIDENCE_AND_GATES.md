# Resvg-Safe SVG Output Pipeline - Evidence And Gates

Status: Active
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

## Notes

- Zed's `crates/mermaid_render` wrapper is GPL-licensed. Treat it as integration evidence only.
- The default `render_svg_sync` parity path must stay unchanged unless a task explicitly says
  otherwise and parity gates are rerun.
