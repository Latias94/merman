# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `c4_medium, xychart_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `c4_medium` | `parse` | 184.45 µs | 23.274 µs | 7.93x |
| `c4_medium` | `layout` | 59.794 µs | 68.357 µs | 0.87x |
| `c4_medium` | `render` | 72.104 µs | 56.492 µs | 1.28x |
| `c4_medium` | `end_to_end` | 329.61 µs | 169.74 µs | 1.94x |
| `xychart_medium` | `parse` | 15.376 µs | 5.5425 µs | 2.77x |
| `xychart_medium` | `layout` | 67.091 µs | 7.4904 µs | 8.96x |
| `xychart_medium` | `render` | 142.50 µs | 32.336 µs | 4.41x |
| `xychart_medium` | `end_to_end` | 226.03 µs | 45.672 µs | 4.95x |

## Summary (geometric mean of ratios)

- `parse`: `4.69x`
- `layout`: `2.80x`
- `render`: `2.37x`
- `end_to_end`: `3.10x`
