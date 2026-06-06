# HPD-090 Raster Baseline Test Hygiene Follow-Up

Date: 2026-06-06

## Trigger

The HPD-080 raster missing-font regression test still embedded `v11.12.2` in its synthetic SVG
text. Runtime Info/Error output and stored Info baselines had already moved to pinned Mermaid
`11.15.0` truth during HPD-090, so leaving the stale literal in a current-facing raster test made
future baseline audits noisier than necessary.

## Change

- Replaced the hardcoded raster test text `v11.12.2` with
  `PINNED_MERMAID_BASELINE_VERSION`.
- Kept the test's assertion unchanged: it still proves that missing requested fonts rasterize with
  visible non-background PNG ink.

## Boundary

This is a test hygiene follow-up only. It does not change parity SVG output, raster output,
fixtures, stored upstream baselines, layout goldens, or root viewport residual status. HPD-090
remains closed; this follow-up only keeps the raster regression's synthetic visible text aligned
with the pinned baseline registry.

## Verification

- `cargo fmt --check -p merman` - passed.
- `cargo nextest run -p merman --features raster render::raster::tests::svg_to_png_keeps_text_visible_when_requested_font_is_missing` -
  passed, `1` test run and `54` skipped.
