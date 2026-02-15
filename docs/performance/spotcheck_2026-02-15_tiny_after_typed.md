# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `20`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `flowchart_tiny, class_tiny, sequence_tiny, state_tiny`

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_tiny` | `parse` | 4.4604 µs | 3.7144 µs | 1.20x |
| `flowchart_tiny` | `layout` | 16.309 µs | 16.990 µs | 0.96x |
| `flowchart_tiny` | `render` | 13.701 µs | 5.1517 µs | 2.66x |
| `flowchart_tiny` | `end_to_end` | 36.790 µs | 38.198 µs | 0.96x |
| `class_tiny` | `parse` | 2.0719 µs | 2.0562 µs | 1.01x |
| `class_tiny` | `layout` | 15.822 µs | 21.670 µs | 0.73x |
| `class_tiny` | `render` | 28.803 µs | 5.6463 µs | 5.10x |
| `class_tiny` | `end_to_end` | 32.412 µs | 19.673 µs | 1.65x |
| `sequence_tiny` | `parse` | 10.340 µs | 1.8289 µs | 5.65x |
| `sequence_tiny` | `layout` | 4.6934 µs | 4.8466 µs | 0.97x |
| `sequence_tiny` | `render` | 8.0866 µs | 8.1636 µs | 0.99x |
| `sequence_tiny` | `end_to_end` | 26.538 µs | 22.237 µs | 1.19x |
| `state_tiny` | `parse` | 4.2944 µs | 2.2992 µs | 1.87x |
| `state_tiny` | `layout` | 15.809 µs | 18.129 µs | 0.87x |
| `state_tiny` | `render` | 8.6821 µs | 5.7056 µs | 1.52x |
| `state_tiny` | `end_to_end` | 33.816 µs | 30.097 µs | 1.12x |

## Summary (geometric mean of ratios)

- `parse`: `1.89x`
- `layout`: `0.88x`
- `render`: `2.13x`
- `end_to_end`: `1.21x`
