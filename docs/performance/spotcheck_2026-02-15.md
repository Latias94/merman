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
| `flowchart_medium` | `parse` | 267.67 µs | 251.74 µs | 1.06x |
| `flowchart_medium` | `layout` | 5.9613 ms | 5.0866 ms | 1.17x |
| `flowchart_medium` | `render` | 373.82 µs | 188.20 µs | 1.99x |
| `flowchart_medium` | `end_to_end` | 5.8573 ms | 6.0118 ms | 0.97x |
| `class_medium` | `parse` | 128.19 µs | 67.498 µs | 1.90x |
| `class_medium` | `layout` | 739.29 µs | 2.0521 ms | 0.36x |
| `class_medium` | `render` | 381.12 µs | 113.44 µs | 3.36x |
| `class_medium` | `end_to_end` | 979.03 µs | 1.7475 ms | 0.56x |
| `sequence_medium` | `parse` | 68.845 µs | 23.867 µs | 2.88x |
| `sequence_medium` | `layout` | 49.784 µs | 119.76 µs | 0.42x |
| `sequence_medium` | `render` | 31.297 µs | 42.330 µs | 0.74x |
| `sequence_medium` | `end_to_end` | 141.81 µs | 181.51 µs | 0.78x |
| `mindmap_medium` | `parse` | 13.961 µs | 10.810 µs | 1.29x |
| `mindmap_medium` | `layout` | 75.176 µs | 28.759 µs | 2.61x |
| `mindmap_medium` | `render` | 50.150 µs | 35.821 µs | 1.40x |
| `mindmap_medium` | `end_to_end` | 150.47 µs | 112.86 µs | 1.33x |
| `architecture_medium` | `parse` | 3.8602 µs | 6.7701 µs | 0.57x |
| `architecture_medium` | `layout` | 40.801 µs | 6.7379 µs | 6.06x |
| `architecture_medium` | `render` | 30.442 µs | 9.7553 µs | 3.12x |
| `architecture_medium` | `end_to_end` | 87.590 µs | 20.648 µs | 4.24x |

## Summary (geometric mean of ratios)

- `parse`: `1.34x`
- `layout`: `1.23x`
- `render`: `1.85x`
- `end_to_end`: `1.19x`
