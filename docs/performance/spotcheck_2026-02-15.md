# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `5s`
- fixtures: `flowchart_medium, class_medium, sequence_medium, mindmap_medium, architecture_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 255.39 µs | 191.18 µs | 1.34x |
| `flowchart_medium` | `layout` | 3.8219 ms | 3.6785 ms | 1.04x |
| `flowchart_medium` | `render` | 226.73 µs | 129.82 µs | 1.75x |
| `flowchart_medium` | `end_to_end` | 4.5954 ms | 5.2917 ms | 0.87x |
| `class_medium` | `parse` | 197.41 µs | 78.994 µs | 2.50x |
| `class_medium` | `layout` | 871.70 µs | 2.2825 ms | 0.38x |
| `class_medium` | `render` | 387.13 µs | 104.74 µs | 3.70x |
| `class_medium` | `end_to_end` | 1.6263 ms | 2.6356 ms | 0.62x |
| `sequence_medium` | `parse` | 94.433 µs | 36.871 µs | 2.56x |
| `sequence_medium` | `layout` | 84.071 µs | 198.79 µs | 0.42x |
| `sequence_medium` | `render` | 51.960 µs | 66.668 µs | 0.78x |
| `sequence_medium` | `end_to_end` | 160.05 µs | 214.14 µs | 0.75x |
| `mindmap_medium` | `parse` | 15.606 µs | 13.510 µs | 1.16x |
| `mindmap_medium` | `layout` | 103.58 µs | 38.808 µs | 2.67x |
| `mindmap_medium` | `render` | 51.420 µs | 37.121 µs | 1.39x |
| `mindmap_medium` | `end_to_end` | 159.58 µs | 82.830 µs | 1.93x |
| `architecture_medium` | `parse` | 2.7223 µs | 3.6187 µs | 0.75x |
| `architecture_medium` | `layout` | 32.296 µs | 7.5889 µs | 4.26x |
| `architecture_medium` | `render` | 33.014 µs | 13.137 µs | 2.51x |
| `architecture_medium` | `end_to_end` | 101.24 µs | 24.318 µs | 4.16x |

## Summary (geometric mean of ratios)

- `parse`: `1.49x`
- `layout`: `1.14x`
- `render`: `1.77x`
- `end_to_end`: `1.26x`
