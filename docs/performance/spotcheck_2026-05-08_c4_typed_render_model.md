# C4 Typed Render Model Spotcheck

This note captures a post-migration spotcheck for the C4 typed render path. The JSON compatibility
path remains stable; this check focuses on the public render API and its typed model dispatch.

## Parameters

- Date: 2026-05-08
- Git state: working tree after the C4 typed-first migration
- Fixture: inline C4 render regression input
- Render path check: `MERMAN_PARSE_TIMING=1 cargo test -p merman --features render c4_render_svg_sync_uses_typed_render_path -- --nocapture`
- JSON parity compare: `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Observations

- The public render path emitted:
  `[parse-render-timing] diagram=c4 model=c4 total=3.6733ms preprocess=1.9159ms parse=1.7461ms sanitize=10µs input_bytes=163`
- The SVG parity compare passed with no DOM drift.
- The public render path now reaches the typed `RenderSemanticModel::C4` variant before layout and SVG
  emission.
- A same-machine before/after Criterion pair is still absent for the already-landed migration, but
  `c4_medium` is benchmarkable through the pipeline bench; see
  `docs/performance/spotcheck_2026-05-09_c4_xychart_pipeline_bench_smoke.md`.
