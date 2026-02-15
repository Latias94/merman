# Stage Spot-check (merman vs mermaid-rs-renderer)

Local stage spot-check captured on 2026-02-15 after merging local `main` and landing architecture
pipeline refactors (typed layout adjacency + typed SVG render without a JSON roundtrip).

This report is intended for quick perf triage (stage attribution), and is **not** a substitute for
high-confidence benchmarking.

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `flowchart_medium, class_medium, sequence_tiny, mindmap_medium, architecture_medium, class_tiny`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 203.47 µs | 169.54 µs | 1.20x |
| `flowchart_medium` | `layout` | 3.5971 ms | 3.2051 ms | 1.12x |
| `flowchart_medium` | `render` | 223.11 µs | 124.12 µs | 1.80x |
| `flowchart_medium` | `end_to_end` | 4.0265 ms | 3.8644 ms | 1.04x |
| `class_medium` | `parse` | 55.638 µs | 43.817 µs | 1.27x |
| `class_medium` | `layout` | 446.36 µs | 1.4075 ms | 0.32x |
| `class_medium` | `render` | 224.22 µs | 69.564 µs | 3.22x |
| `class_medium` | `end_to_end` | 772.54 µs | 1.5698 ms | 0.49x |
| `sequence_tiny` | `parse` | 6.8337 µs | 1.6073 µs | 4.25x |
| `sequence_tiny` | `layout` | 4.6517 µs | 4.5344 µs | 1.03x |
| `sequence_tiny` | `render` | 7.7555 µs | 7.5714 µs | 1.02x |
| `sequence_tiny` | `end_to_end` | 19.526 µs | 14.515 µs | 1.35x |
| `mindmap_medium` | `parse` | 13.815 µs | 10.103 µs | 1.37x |
| `mindmap_medium` | `layout` | 70.162 µs | 28.172 µs | 2.49x |
| `mindmap_medium` | `render` | 44.263 µs | 32.681 µs | 1.35x |
| `mindmap_medium` | `end_to_end` | 133.34 µs | 73.059 µs | 1.83x |
| `architecture_medium` | `parse` | 2.4061 µs | 3.2936 µs | 0.73x |
| `architecture_medium` | `layout` | 35.742 µs | 4.4243 µs | 8.08x |
| `architecture_medium` | `render` | 18.848 µs | 10.385 µs | 1.81x |
| `architecture_medium` | `end_to_end` | 67.598 µs | 16.344 µs | 4.14x |
| `class_tiny` | `parse` | 1.9501 µs | 2.0733 µs | 0.94x |
| `class_tiny` | `layout` | 15.462 µs | 11.567 µs | 1.34x |
| `class_tiny` | `render` | 15.005 µs | 4.9931 µs | 3.01x |
| `class_tiny` | `end_to_end` | 31.143 µs | 18.229 µs | 1.71x |

## Summary (geometric mean of ratios)

- `parse`: `1.35x`
- `layout`: `1.46x`
- `render`: `1.88x`
- `end_to_end`: `1.44x`

