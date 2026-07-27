# XYChart Render Allocation Cleanup Spot-check

This note records a small Criterion smoke run after reducing allocation overhead in the XYChart SVG
renderer. The sample size is intentionally low; use this as local regression evidence, not as a
release-grade benchmark.

## Parameters

- Date: 2026-05-09
- Git state: working tree after the XYChart SVG allocation cleanup
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 "xychart_medium"`

## Code Change

- `svg/parity/xychart.rs` now stores temporary DOM nodes with static tag names and insertion-order
  attribute vectors instead of allocating per-node tag `String`s and `BTreeMap` attribute tables.
- Repeated XYChart `getGroup()` path construction now goes through one parent/class-keyed helper,
  matching the nested group ownership model without three local copies of the same loop.
- XYChart shared CSS now writes directly into the output SVG buffer through `push_xychart_css`,
  avoiding a temporary CSS `String` and a second copy into the final SVG.

## Results

| benchmark | observed time range |
| --- | ---: |
| `parse/xychart_medium` | 13.961-15.640 us |
| `parse_known_type/xychart_medium` | 60.523-66.040 us |
| `layout/xychart_medium` | 65.808-72.951 us |
| `render/xychart_medium` | 113.74-122.92 us |
| `end_to_end/xychart_medium` | 172.27-190.32 us |

Criterion's local change analysis reported `render/xychart_medium` at `-20.615%..-8.8182%`
with a `-14.810%` midpoint and `p = 0.00`. Other stages did not show a statistically meaningful
change in this smoke run.

For rough continuity with the earlier same-day smoke in
`spotcheck_2026-05-09_c4_xychart_pipeline_bench_smoke.md`, `render/xychart_medium` moved from
`128.08-143.28 us` to `113.74-122.92 us`. That comparison is useful as a direction check, but it
is not an isolated before/after benchmark.

## Observations

- The cleanup reduces SVG render allocation overhead without changing XYChart DOM parity.
- End-to-end movement was not statistically confirmed in this low-sample smoke, so the remaining
  next targets stay `xychart` layout and the still-large cross-repo render gap.
- The verification gate for this cleanup was:
  `cargo nextest run -p merman-render xychart`,
  `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, and
  `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`.
