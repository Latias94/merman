# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution), not as a final benchmark.

## Parameters

- date: `2026-02-16`
- script: `python tools/bench/stage_spotcheck.py`
- sample-size: `20`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `flowchart_medium, class_medium, architecture_medium, mindmap_medium, flowchart_tiny, state_tiny, sequence_tiny`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 271.38 µs | 179.16 µs | 1.51x |
| `flowchart_medium` | `layout` | 5.1215 ms | 4.5544 ms | 1.12x |
| `flowchart_medium` | `render` | 290.74 µs | 131.92 µs | 2.20x |
| `flowchart_medium` | `end_to_end` | 4.7981 ms | 5.3027 ms | 0.90x |
| `class_medium` | `parse` | 77.979 µs | 44.417 µs | 1.76x |
| `class_medium` | `layout` | 520.43 µs | 1.4200 ms | 0.37x |
| `class_medium` | `render` | 252.97 µs | 70.916 µs | 3.57x |
| `class_medium` | `end_to_end` | 1.0984 ms | 1.9893 ms | 0.55x |
| `architecture_medium` | `parse` | 2.4880 µs | 3.3094 µs | 0.75x |
| `architecture_medium` | `layout` | 13.581 µs | 4.6636 µs | 2.91x |
| `architecture_medium` | `render` | 18.494 µs | 7.6445 µs | 2.42x |
| `architecture_medium` | `end_to_end` | 34.973 µs | 16.386 µs | 2.13x |
| `mindmap_medium` | `parse` | 19.293 µs | 10.221 µs | 1.89x |
| `mindmap_medium` | `layout` | 68.696 µs | 28.376 µs | 2.42x |
| `mindmap_medium` | `render` | 60.307 µs | 33.114 µs | 1.82x |
| `mindmap_medium` | `end_to_end` | 132.71 µs | 74.323 µs | 1.79x |
| `flowchart_tiny` | `parse` | 4.3589 µs | 3.9866 µs | 1.09x |
| `flowchart_tiny` | `layout` | 24.040 µs | 39.403 µs | 0.61x |
| `flowchart_tiny` | `render` | 29.236 µs | 12.825 µs | 2.28x |
| `flowchart_tiny` | `end_to_end` | 65.044 µs | 42.248 µs | 1.54x |
| `state_tiny` | `parse` | 8.1467 µs | 2.0155 µs | 4.04x |
| `state_tiny` | `layout` | 22.111 µs | 16.383 µs | 1.35x |
| `state_tiny` | `render` | 8.9729 µs | 4.1206 µs | 2.18x |
| `state_tiny` | `end_to_end` | 31.139 µs | 28.104 µs | 1.11x |
| `sequence_tiny` | `parse` | 9.3022 µs | 2.4530 µs | 3.79x |
| `sequence_tiny` | `layout` | 6.7297 µs | 5.9237 µs | 1.14x |
| `sequence_tiny` | `render` | 9.0082 µs | 8.5720 µs | 1.05x |
| `sequence_tiny` | `end_to_end` | 18.608 µs | 15.490 µs | 1.20x |

## Summary (geometric mean of ratios)

- `parse`: `1.81x`
- `layout`: `1.15x`
- `render`: `2.10x`
- `end_to_end`: `1.21x`

## Notes

- This run includes recent work:
  - mindmap: indexed COSE layout to avoid string-key position maps
  - architecture: avoid cloning effective config in SVG render hot path
