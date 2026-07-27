# C4 and XYChart Pipeline Bench Smoke

This note records a small Criterion smoke run that verifies `c4_medium` and `xychart_medium` are
benchmarkable through the standard `pipeline` bench. The sample size is intentionally low; use this
as fixture/coverage evidence, not as a release-grade performance verdict.

## Parameters

- Date: 2026-05-09
- Git state: working tree after the C4 direct render-model parse cleanup
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 "c4_medium|xychart_medium"`

## Results

| benchmark | observed time range |
| --- | ---: |
| `parse/c4_medium` | 38.262-42.727 us |
| `parse_known_type/c4_medium` | 182.60-211.94 us |
| `layout/c4_medium` | 53.413-62.361 us |
| `render/c4_medium` | 75.433-84.318 us |
| `end_to_end/c4_medium` | 184.46-221.01 us |
| `parse/xychart_medium` | 12.547-14.215 us |
| `parse_known_type/xychart_medium` | 56.487-67.796 us |
| `layout/xychart_medium` | 60.561-69.852 us |
| `render/xychart_medium` | 128.08-143.28 us |
| `end_to_end/xychart_medium` | 229.41-273.99 us |

## Observations

- `c4_medium` already runs through parse, known-type parse, layout, render, and end-to-end stages.
- `xychart_medium` no longer hits the bench pre-check skip path after switching the fixture to the
  upstream bracketed axis syntax that the current parser supports.
- Criterion may print change/regression lines against whatever local baseline exists under
  `target/criterion`; those lines were ignored for this smoke because the run used a deliberately
  small sample size.
- A true before/after Criterion pair remains a future requirement for the next typed migration or a
  dedicated historical checkout comparison.
