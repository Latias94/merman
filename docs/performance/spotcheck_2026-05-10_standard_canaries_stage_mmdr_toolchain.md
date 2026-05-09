# Standard Canary Stage Spot-check

This report records a current stage-attribution run for the standard performance canaries after
making the benchmark helper scripts lockfile-aware and adding an explicit `--mmdr-toolchain`
parameter for the local `mermaid-rs-renderer` checkout.

Command:

```bash
python tools/bench/stage_spotcheck.py --preset long --mmdr-toolchain 1.92.0 --fixtures flowchart_medium,class_medium,mindmap_medium,architecture_medium --out target/bench/stage_standard_canaries_latest.md
```

## Parameters

- sample-size: `30`
- warm-up: `2s`
- measurement: `3s`
- mmdr-toolchain: `1.92.0`
- fixtures: `flowchart_medium, class_medium, mindmap_medium, architecture_medium`

## Results

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `flowchart_medium` | `parse` | 353.28 us | 393.56 us | 0.90x |
| `flowchart_medium` | `layout` | 7.3054 ms | 12.640 ms | 0.58x |
| `flowchart_medium` | `render` | 454.40 us | 218.29 us | 2.08x |
| `flowchart_medium` | `end_to_end` | 7.2751 ms | 14.272 ms | 0.51x |
| `class_medium` | `parse` | 101.57 us | 104.09 us | 0.98x |
| `class_medium` | `layout` | 1.0896 ms | 2.7782 ms | 0.39x |
| `class_medium` | `render` | 566.53 us | 191.79 us | 2.95x |
| `class_medium` | `end_to_end` | 1.8955 ms | 3.0310 ms | 0.63x |
| `mindmap_medium` | `parse` | 25.466 us | 20.988 us | 1.21x |
| `mindmap_medium` | `layout` | 164.79 us | 70.286 us | 2.34x |
| `mindmap_medium` | `render` | 84.921 us | 70.118 us | 1.21x |
| `mindmap_medium` | `end_to_end` | 288.80 us | 167.30 us | 1.73x |
| `architecture_medium` | `parse` | 4.5370 us | 6.8586 us | 0.66x |
| `architecture_medium` | `layout` | 114.43 us | 12.118 us | 9.44x |
| `architecture_medium` | `render` | 49.364 us | 19.241 us | 2.57x |
| `architecture_medium` | `end_to_end` | 189.42 us | 44.819 us | 4.23x |

## Summary

- `parse`: `0.92x`
- `layout`: `1.50x`
- `render`: `2.09x`
- `end_to_end`: `1.23x`

## Readout

The current optimization signal is still clear: `architecture_medium` layout dominates the largest
gap, while cross-diagram render fixed-cost remains visible on `flowchart_medium`, `class_medium`,
and `architecture_medium`. Flowchart and class remain favorable end-to-end canaries despite render
stage gaps because their local layout stages are substantially faster than mmdr in this run.
