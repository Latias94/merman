# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `5s`
- fixtures: `class_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `class_medium` | `parse` | 70.820 µs | 49.050 µs | 1.44x |
| `class_medium` | `layout` | 536.20 µs | 1.7341 ms | 0.31x |
| `class_medium` | `render` | 269.96 µs | 104.96 µs | 2.57x |
| `class_medium` | `end_to_end` | 934.64 µs | 2.0101 ms | 0.46x |

## Summary (geometric mean of ratios)

- `parse`: `1.44x`
- `layout`: `0.31x`
- `render`: `2.57x`
- `end_to_end`: `0.46x`
