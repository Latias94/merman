# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `20`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `flowchart_tiny, sequence_tiny, state_tiny, class_tiny`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_tiny` | `parse` | 4.7153 µs | 4.4884 µs | 1.05x |
| `flowchart_tiny` | `layout` | 17.319 µs | 20.026 µs | 0.86x |
| `flowchart_tiny` | `render` | 10.555 µs | 5.1782 µs | 2.04x |
| `flowchart_tiny` | `end_to_end` | 36.992 µs | 36.552 µs | 1.01x |
| `sequence_tiny` | `parse` | 3.7759 µs | 2.0095 µs | 1.88x |
| `sequence_tiny` | `layout` | 10.328 µs | 5.7910 µs | 1.78x |
| `sequence_tiny` | `render` | 23.609 µs | 17.022 µs | 1.39x |
| `sequence_tiny` | `end_to_end` | 33.288 µs | 31.096 µs | 1.07x |
| `state_tiny` | `parse` | 10.917 µs | 4.1855 µs | 2.61x |
| `state_tiny` | `layout` | 41.453 µs | 46.292 µs | 0.90x |
| `state_tiny` | `render` | 23.040 µs | 8.1410 µs | 2.83x |
| `state_tiny` | `end_to_end` | 77.616 µs | 62.792 µs | 1.24x |
| `class_tiny` | `parse` | 5.1197 µs | 5.4387 µs | 0.94x |
| `class_tiny` | `layout` | 45.107 µs | 34.572 µs | 1.30x |
| `class_tiny` | `render` | 27.078 µs | 12.885 µs | 2.10x |
| `class_tiny` | `end_to_end` | 74.745 µs | 55.918 µs | 1.34x |

## Summary (geometric mean of ratios)

- `parse`: `1.48x`
- `layout`: `1.16x`
- `render`: `2.02x`
- `end_to_end`: `1.16x`
