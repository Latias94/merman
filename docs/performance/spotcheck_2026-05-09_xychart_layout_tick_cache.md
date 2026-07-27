# XYChart Layout Tick Cache Spot-check

This note records a small Criterion smoke run after caching XYChart axis tick labels inside the
layout axis state. The sample size is intentionally low; use this as local regression evidence, not
as a release-grade benchmark.

## Parameters

- Date: 2026-05-09
- Git state: working tree after the XYChart axis tick cache cleanup
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 "xychart_medium"`

## Code Change

- `Axis` now owns the current tick label vector and refreshes it when the axis position changes.
- `calculate_space`, `tick_distance`, and axis drawable generation borrow the cached tick slice
  instead of rebuilding labels for every measurement, tick, and label pass.
- The cached labels preserve the existing left-axis linear-domain reversal, so DOM parity remains
  unchanged.

## Results

| benchmark | observed time range |
| --- | ---: |
| `parse/xychart_medium` | 17.433-19.538 us |
| `parse_known_type/xychart_medium` | 58.096-61.035 us |
| `layout/xychart_medium` | 55.129-60.551 us |
| `render/xychart_medium` | 108.26-124.28 us |
| `end_to_end/xychart_medium` | 186.10-239.72 us |

Criterion's local change analysis reported `layout/xychart_medium` at `-19.458%..-6.6428%`
with a `-13.698%` midpoint and `p = 0.00`. `render/xychart_medium` did not show a statistically
meaningful change in this smoke run.

The same low-sample run reported mixed parse and end-to-end movement. Treat those as noise until a
larger isolated before/after run is needed; the actionable signal here is the isolated layout-stage
improvement.

## Verification

- `cargo nextest run -p merman-render xychart`
- `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`
