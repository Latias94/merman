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
| `flowchart_medium` | `parse` | 266.76 µs | 252.02 µs | 1.06x |
| `flowchart_medium` | `layout` | 6.0700 ms | 4.0913 ms | 1.48x |
| `flowchart_medium` | `render` | 349.85 µs | 180.89 µs | 1.93x |
| `flowchart_medium` | `end_to_end` | 5.6476 ms | 3.9120 ms | 1.44x |
| `class_medium` | `parse` | 111.24 µs | 53.778 µs | 2.07x |
| `class_medium` | `layout` | 683.40 µs | 1.9922 ms | 0.34x |
| `class_medium` | `render` | 361.91 µs | 101.51 µs | 3.57x |
| `class_medium` | `end_to_end` | 1.1007 ms | 2.4195 ms | 0.45x |
| `sequence_medium` | `parse` | 62.671 µs | 25.284 µs | 2.48x |
| `sequence_medium` | `layout` | 70.034 µs | 134.25 µs | 0.52x |
| `sequence_medium` | `render` | 32.215 µs | 42.287 µs | 0.76x |
| `sequence_medium` | `end_to_end` | 153.25 µs | 185.74 µs | 0.83x |
| `mindmap_medium` | `parse` | 15.743 µs | 11.325 µs | 1.39x |
| `mindmap_medium` | `layout` | 99.227 µs | 28.741 µs | 3.45x |
| `mindmap_medium` | `render` | 58.267 µs | 34.461 µs | 1.69x |
| `mindmap_medium` | `end_to_end` | 164.63 µs | 91.302 µs | 1.80x |
| `architecture_medium` | `parse` | 2.7213 µs | 3.3602 µs | 0.81x |
| `architecture_medium` | `layout` | 35.735 µs | 6.0961 µs | 5.86x |
| `architecture_medium` | `render` | 31.359 µs | 11.471 µs | 2.73x |
| `architecture_medium` | `end_to_end` | 93.421 µs | 24.810 µs | 3.77x |

## Summary (geometric mean of ratios)

- `parse`: `1.44x`
- `layout`: `1.40x`
- `render`: `1.89x`
- `end_to_end`: `1.30x`
