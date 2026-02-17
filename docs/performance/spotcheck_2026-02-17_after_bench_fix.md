# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `15`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `flowchart_medium, class_medium, mindmap_medium, architecture_medium`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 205.43 µs | 171.44 µs | 1.20x |
| `flowchart_medium` | `layout` | 3.5606 ms | 3.3197 ms | 1.07x |
| `flowchart_medium` | `render` | 173.38 µs | 125.01 µs | 1.39x |
| `flowchart_medium` | `end_to_end` | 3.8722 ms | 3.7450 ms | 1.03x |
| `class_medium` | `parse` | 58.152 µs | 44.323 µs | 1.31x |
| `class_medium` | `layout` | 448.60 µs | 2.4569 ms | 0.18x |
| `class_medium` | `render` | 214.81 µs | 138.15 µs | 1.55x |
| `class_medium` | `end_to_end` | 1.3181 ms | 1.6004 ms | 0.82x |
| `mindmap_medium` | `parse` | 17.489 µs | 12.562 µs | 1.39x |
| `mindmap_medium` | `layout` | 67.234 µs | 34.697 µs | 1.94x |
| `mindmap_medium` | `render` | 50.152 µs | 34.259 µs | 1.46x |
| `mindmap_medium` | `end_to_end` | 123.04 µs | 77.945 µs | 1.58x |
| `architecture_medium` | `parse` | 2.8615 µs | 4.7014 µs | 0.61x |
| `architecture_medium` | `layout` | 20.182 µs | 4.9763 µs | 4.06x |
| `architecture_medium` | `render` | 44.499 µs | 30.554 µs | 1.46x |
| `architecture_medium` | `end_to_end` | 75.438 µs | 21.182 µs | 3.56x |

## Summary (geometric mean of ratios)

- `parse`: `1.07x`
- `layout`: `1.11x`
- `render`: `1.46x`
- `end_to_end`: `1.48x`
