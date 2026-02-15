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
| `flowchart_tiny` | `parse` | 4.3924 µs | 3.6037 µs | 1.22x |
| `flowchart_tiny` | `layout` | 16.277 µs | 16.994 µs | 0.96x |
| `flowchart_tiny` | `render` | 12.691 µs | 4.9036 µs | 2.59x |
| `flowchart_tiny` | `end_to_end` | 35.504 µs | 28.793 µs | 1.23x |
| `class_tiny` | `parse` | 5.1241 µs | 1.9915 µs | 2.57x |
| `class_tiny` | `layout` | 15.919 µs | 11.264 µs | 1.41x |
| `class_tiny` | `render` | 14.569 µs | 4.3198 µs | 3.37x |
| `class_tiny` | `end_to_end` | 35.388 µs | 18.475 µs | 1.92x |
| `sequence_tiny` | `parse` | 7.1288 µs | 1.6098 µs | 4.43x |
| `sequence_tiny` | `layout` | 5.0469 µs | 4.6003 µs | 1.10x |
| `sequence_tiny` | `render` | 7.8799 µs | 8.9254 µs | 0.88x |
| `sequence_tiny` | `end_to_end` | 22.890 µs | 15.638 µs | 1.46x |
| `state_tiny` | `parse` | 4.0423 µs | 1.9040 µs | 2.12x |
| `state_tiny` | `layout` | 15.062 µs | 15.796 µs | 0.95x |
| `state_tiny` | `render` | 16.647 µs | 3.8091 µs | 4.37x |
| `state_tiny` | `end_to_end` | 35.828 µs | 21.893 µs | 1.64x |

## Summary (geometric mean of ratios)

- `parse`: `2.33x`
- `layout`: `1.09x`
- `render`: `2.41x`
- `end_to_end`: `1.54x`

