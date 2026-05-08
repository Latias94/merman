# C4 and XYChart Pipeline Bench Smoke

This note records a small Criterion smoke run that verifies `c4_medium` and `xychart_medium` are
benchmarkable through the standard `pipeline` bench. The sample size is intentionally low; use this
as fixture/coverage evidence, not as a release-grade performance verdict.

## Parameters

- Date: 2026-05-09
- Git state: working tree after the override helper/gate refactors and the `xychart_medium` fixture
  repair
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 c4_medium`
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 xychart_medium`

## Results

| benchmark | observed time range |
| --- | ---: |
| `parse/c4_medium` | 207.87-241.46 us |
| `parse_known_type/c4_medium` | 165.95-187.03 us |
| `layout/c4_medium` | 54.468-65.543 us |
| `render/c4_medium` | 76.782-91.130 us |
| `end_to_end/c4_medium` | 301.68-352.39 us |
| `parse/xychart_medium` | 13.684-16.279 us |
| `parse_known_type/xychart_medium` | 55.946-59.308 us |
| `layout/xychart_medium` | 64.054-73.375 us |
| `render/xychart_medium` | 164.12-182.26 us |
| `end_to_end/xychart_medium` | 229.76-261.34 us |

## Observations

- `c4_medium` already runs through parse, known-type parse, layout, render, and end-to-end stages.
- `xychart_medium` no longer hits the bench pre-check skip path after switching the fixture to the
  upstream bracketed axis syntax that the current parser supports.
- Criterion may print change/regression lines against whatever local baseline exists under
  `target/criterion`; those lines were ignored for this smoke because the run used a deliberately
  small sample size.
- A true before/after Criterion pair remains a future requirement for the next typed migration or a
  dedicated historical checkout comparison.
