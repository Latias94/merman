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
| `flowchart_medium` | `parse` | 304.05 µs | 181.77 µs | 1.67x |
| `flowchart_medium` | `layout` | 4.1753 ms | 3.5951 ms | 1.16x |
| `flowchart_medium` | `render` | 307.98 µs | 149.94 µs | 2.05x |
| `flowchart_medium` | `end_to_end` | 5.1448 ms | 5.2727 ms | 0.98x |
| `class_medium` | `parse` | 88.831 µs | 46.269 µs | 1.92x |
| `class_medium` | `layout` | 478.85 µs | 1.5517 ms | 0.31x |
| `class_medium` | `render` | 179.20 µs | 92.861 µs | 1.93x |
| `class_medium` | `end_to_end` | 1.1678 ms | 2.8426 ms | 0.41x |
| `mindmap_medium` | `parse` | 15.448 µs | 10.944 µs | 1.41x |
| `mindmap_medium` | `layout` | 91.950 µs | 31.414 µs | 2.93x |
| `mindmap_medium` | `render` | 63.815 µs | 43.532 µs | 1.47x |
| `mindmap_medium` | `end_to_end` | 217.47 µs | 100.46 µs | 2.16x |
| `architecture_medium` | `parse` | 3.4011 µs | 4.4582 µs | 0.76x |
| `architecture_medium` | `layout` | 19.869 µs | 5.1504 µs | 3.86x |
| `architecture_medium` | `render` | 26.481 µs | 10.795 µs | 2.45x |
| `architecture_medium` | `end_to_end` | 48.648 µs | 17.893 µs | 2.72x |

## Summary (geometric mean of ratios)

- `parse`: `1.36x`
- `layout`: `1.42x`
- `render`: `1.94x`
- `end_to_end`: `1.24x`
