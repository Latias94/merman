# XYChart Typed Render Model Spotcheck

This note captures a post-migration spotcheck for the xychart typed render path. The JSON
compatibility path remains stable; this check focuses on the public render API and its typed model
dispatch.

## Parameters

- Date: 2026-05-08
- Git state: working tree after the xychart typed-first migration
- Fixture: inline xychart render regression input
- Render path check: `cargo test -p merman --features render xychart_render_svg_sync_uses_typed_render_path -- --nocapture`
- JSON parity compare: `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Observations

- The public render path emitted:
  `[parse-render-timing] diagram=xychart model=xychart total=2.5699ms preprocess=2.1716ms parse=382.7µs sanitize=14.2µs input_bytes=71`
- The SVG parity compare passed with no DOM drift.
- The JSON compatibility path still parses and renders xychart fixtures through the existing parity
  compare command.
- `xychart_medium` remains skipped by the filtered pipeline bench pre-checks, so a Criterion
  before/after pair is still pending for a future benchmarkable fixture.
