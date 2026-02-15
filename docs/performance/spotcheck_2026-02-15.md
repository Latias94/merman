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
| `flowchart_medium` | `parse` | 328.65 µs | 272.10 µs | 1.21x |
| `flowchart_medium` | `layout` | 6.3124 ms | 5.2699 ms | 1.20x |
| `flowchart_medium` | `render` | 372.87 µs | 175.50 µs | 2.12x |
| `flowchart_medium` | `end_to_end` | 6.5480 ms | 6.3492 ms | 1.03x |
| `class_medium` | `parse` | 158.19 µs | 67.319 µs | 2.35x |
| `class_medium` | `layout` | 770.99 µs | 2.2277 ms | 0.35x |
| `class_medium` | `render` | 399.51 µs | 107.69 µs | 3.71x |
| `class_medium` | `end_to_end` | 1.3940 ms | 3.6113 ms | 0.39x |
| `sequence_medium` | `parse` | 93.024 µs | 35.011 µs | 2.66x |
| `sequence_medium` | `layout` | 77.885 µs | 194.91 µs | 0.40x |
| `sequence_medium` | `render` | 48.339 µs | 61.762 µs | 0.78x |
| `sequence_medium` | `end_to_end` | 203.39 µs | 295.36 µs | 0.69x |
| `mindmap_medium` | `parse` | 22.966 µs | 15.483 µs | 1.48x |
| `mindmap_medium` | `layout` | 163.22 µs | 40.084 µs | 4.07x |
| `mindmap_medium` | `render` | 76.363 µs | 49.544 µs | 1.54x |
| `mindmap_medium` | `end_to_end` | 263.38 µs | 114.42 µs | 2.30x |
| `architecture_medium` | `parse` | 3.9750 µs | 4.9450 µs | 0.80x |
| `architecture_medium` | `layout` | 44.249 µs | 6.6068 µs | 6.70x |
| `architecture_medium` | `render` | 34.451 µs | 13.847 µs | 2.49x |
| `architecture_medium` | `end_to_end` | 93.038 µs | 25.879 µs | 3.60x |

## Summary (geometric mean of ratios)

- `parse`: `1.55x`
- `layout`: `1.35x`
- `render`: `1.88x`
- `end_to_end`: `1.18x`
