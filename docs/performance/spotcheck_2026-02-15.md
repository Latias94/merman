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
| `flowchart_medium` | `parse` | 323.64 µs | 282.55 µs | 1.15x |
| `flowchart_medium` | `layout` | 5.1267 ms | 5.1506 ms | 1.00x |
| `flowchart_medium` | `render` | 365.54 µs | 177.73 µs | 2.06x |
| `flowchart_medium` | `end_to_end` | 5.8910 ms | 6.1737 ms | 0.95x |
| `class_medium` | `parse` | 157.64 µs | 66.361 µs | 2.38x |
| `class_medium` | `layout` | 625.89 µs | 1.5408 ms | 0.41x |
| `class_medium` | `render` | 292.44 µs | 105.94 µs | 2.76x |
| `class_medium` | `end_to_end` | 1.2699 ms | 2.3088 ms | 0.55x |
| `sequence_medium` | `parse` | 84.889 µs | 26.821 µs | 3.17x |
| `sequence_medium` | `layout` | 62.142 µs | 128.91 µs | 0.48x |
| `sequence_medium` | `render` | 33.397 µs | 42.381 µs | 0.79x |
| `sequence_medium` | `end_to_end` | 140.46 µs | 178.01 µs | 0.79x |
| `mindmap_medium` | `parse` | 14.829 µs | 10.569 µs | 1.40x |
| `mindmap_medium` | `layout` | 74.109 µs | 28.823 µs | 2.57x |
| `mindmap_medium` | `render` | 45.740 µs | 33.798 µs | 1.35x |
| `mindmap_medium` | `end_to_end` | 155.83 µs | 78.880 µs | 1.98x |
| `architecture_medium` | `parse` | 2.7067 µs | 3.0988 µs | 0.87x |
| `architecture_medium` | `layout` | 27.641 µs | 4.6880 µs | 5.90x |
| `architecture_medium` | `render` | 22.634 µs | 8.0039 µs | 2.83x |
| `architecture_medium` | `end_to_end` | 57.478 µs | 19.213 µs | 2.99x |

## Summary (geometric mean of ratios)

- `parse`: `1.60x`
- `layout`: `1.24x`
- `render`: `1.76x`
- `end_to_end`: `1.20x`
