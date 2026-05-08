# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `30`
- warm-up: `2s`
- measurement: `3s`
- fixtures: `c4_medium, xychart_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `c4_medium` | `parse` | 41.605 µs | 23.268 µs | 1.79x |
| `c4_medium` | `layout` | 61.732 µs | 63.839 µs | 0.97x |
| `c4_medium` | `render` | 80.003 µs | 53.972 µs | 1.48x |
| `c4_medium` | `end_to_end` | 205.18 µs | 152.71 µs | 1.34x |
| `xychart_medium` | `parse` | 14.975 µs | 5.5426 µs | 2.70x |
| `xychart_medium` | `layout` | 62.528 µs | 7.3276 µs | 8.53x |
| `xychart_medium` | `render` | 138.94 µs | 35.019 µs | 3.97x |
| `xychart_medium` | `end_to_end` | 233.21 µs | 51.052 µs | 4.57x |

## Summary (geometric mean of ratios)

- `parse`: `2.20x`
- `layout`: `2.87x`
- `render`: `2.43x`
- `end_to_end`: `2.48x`
