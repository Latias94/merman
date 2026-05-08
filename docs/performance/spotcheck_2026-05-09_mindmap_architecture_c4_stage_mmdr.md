# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `30`
- warm-up: `2s`
- measurement: `3s`
- fixtures: `mindmap_medium, architecture_medium, c4_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `mindmap_medium` | `parse` | 25.087 µs | 21.897 µs | 1.15x |
| `mindmap_medium` | `layout` | 150.87 µs | 66.233 µs | 2.28x |
| `mindmap_medium` | `render` | 78.003 µs | 72.428 µs | 1.08x |
| `mindmap_medium` | `end_to_end` | 308.34 µs | 171.89 µs | 1.79x |
| `architecture_medium` | `parse` | 4.7384 µs | 6.7027 µs | 0.71x |
| `architecture_medium` | `layout` | 106.93 µs | 11.980 µs | 8.93x |
| `architecture_medium` | `render` | 50.897 µs | 21.051 µs | 2.42x |
| `architecture_medium` | `end_to_end` | 180.40 µs | 41.461 µs | 4.35x |
| `c4_medium` | `parse` | 42.654 µs | 20.974 µs | 2.03x |
| `c4_medium` | `layout` | 56.149 µs | 63.992 µs | 0.88x |
| `c4_medium` | `render` | 81.006 µs | 52.648 µs | 1.54x |
| `c4_medium` | `end_to_end` | 202.18 µs | 143.77 µs | 1.41x |

## Summary (geometric mean of ratios)

- `parse`: `1.18x`
- `layout`: `2.61x`
- `render`: `1.59x`
- `end_to_end`: `2.22x`
