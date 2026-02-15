# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `3s`
- fixtures: `flowchart_medium, class_medium, sequence_medium, mindmap_medium, architecture_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 332.75 µs | 310.65 µs | 1.07x |
| `flowchart_medium` | `layout` | 5.8928 ms | 4.5175 ms | 1.30x |
| `flowchart_medium` | `render` | 339.61 µs | 191.96 µs | 1.77x |
| `flowchart_medium` | `end_to_end` | 5.6219 ms | 5.4696 ms | 1.03x |
| `class_medium` | `parse` | 131.27 µs | 60.123 µs | 2.18x |
| `class_medium` | `layout` | 602.64 µs | 2.0291 ms | 0.30x |
| `class_medium` | `render` | 344.78 µs | 85.874 µs | 4.01x |
| `class_medium` | `end_to_end` | 1.0118 ms | 1.8941 ms | 0.53x |
| `sequence_medium` | `parse` | 67.425 µs | 22.069 µs | 3.06x |
| `sequence_medium` | `layout` | 50.350 µs | 112.12 µs | 0.45x |
| `sequence_medium` | `render` | 33.051 µs | 39.556 µs | 0.84x |
| `sequence_medium` | `end_to_end` | 142.71 µs | 199.08 µs | 0.72x |
| `mindmap_medium` | `parse` | 17.705 µs | 14.349 µs | 1.23x |
| `mindmap_medium` | `layout` | 124.79 µs | 36.217 µs | 3.45x |
| `mindmap_medium` | `render` | 67.422 µs | 36.403 µs | 1.85x |
| `mindmap_medium` | `end_to_end` | 163.33 µs | 78.472 µs | 2.08x |
| `architecture_medium` | `parse` | 2.9515 µs | 5.0608 µs | 0.58x |
| `architecture_medium` | `layout` | 29.657 µs | 4.7910 µs | 6.19x |
| `architecture_medium` | `render` | 23.527 µs | 15.145 µs | 1.55x |
| `architecture_medium` | `end_to_end` | 74.903 µs | 24.007 µs | 3.12x |

## Summary (geometric mean of ratios)

- `parse`: `1.39x`
- `layout`: `1.30x`
- `render`: `1.76x`
- `end_to_end`: `1.21x`
