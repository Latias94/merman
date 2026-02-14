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
| `flowchart_medium` | `parse` | 372.95 µs | 287.15 µs | 1.30x |
| `flowchart_medium` | `layout` | 6.8023 ms | 4.3461 ms | 1.57x |
| `flowchart_medium` | `render` | 374.17 µs | 197.23 µs | 1.90x |
| `flowchart_medium` | `end_to_end` | 7.6393 ms | 6.2078 ms | 1.23x |
| `class_medium` | `parse` | 170.06 µs | 73.659 µs | 2.31x |
| `class_medium` | `layout` | 774.17 µs | 1.6226 ms | 0.48x |
| `class_medium` | `render` | 399.79 µs | 88.699 µs | 4.51x |
| `class_medium` | `end_to_end` | 1.4515 ms | 2.8149 ms | 0.52x |
| `sequence_medium` | `parse` | 102.23 µs | 36.190 µs | 2.82x |
| `sequence_medium` | `layout` | 78.706 µs | 159.66 µs | 0.49x |
| `sequence_medium` | `render` | 52.855 µs | 63.515 µs | 0.83x |
| `sequence_medium` | `end_to_end` | 242.61 µs | 306.33 µs | 0.79x |
| `mindmap_medium` | `parse` | 24.698 µs | 16.878 µs | 1.46x |
| `mindmap_medium` | `layout` | 169.04 µs | 35.222 µs | 4.80x |
| `mindmap_medium` | `render` | 77.261 µs | 54.725 µs | 1.41x |
| `mindmap_medium` | `end_to_end` | 294.92 µs | 133.48 µs | 2.21x |
| `architecture_medium` | `parse` | 4.2188 µs | 5.2470 µs | 0.80x |
| `architecture_medium` | `layout` | 46.271 µs | 6.9741 µs | 6.63x |
| `architecture_medium` | `render` | 48.619 µs | 14.167 µs | 3.43x |
| `architecture_medium` | `end_to_end` | 91.613 µs | 27.455 µs | 3.34x |

## Summary (geometric mean of ratios)

- `parse`: `1.58x`
- `layout`: `1.64x`
- `render`: `2.03x`
- `end_to_end`: `1.30x`
