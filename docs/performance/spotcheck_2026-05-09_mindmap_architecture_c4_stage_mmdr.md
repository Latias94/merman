# Stage Spot-check (merman vs mermaid-rs-renderer)

This report is intended for quick perf triage (stage attribution).

## Parameters

- sample-size: `10`
- warm-up: `1s`
- measurement: `1s`
- fixtures: `mindmap_medium, architecture_medium, c4_medium`
- toolchain: `RUSTUP_TOOLCHAIN=1.92.0-x86_64-pc-windows-msvc` for both repositories

Note: the local `repo-ref/mermaid-rs-renderer` dependency graph currently pulls `json5 1.3.1`,
which requires a newer toolchain than the workspace MSRV toolchain used for merman gates.

## Results (mid estimate)

| fixture | stage | merman | mmdr | ratio |
|---|---|---:|---:|---:|
| `mindmap_medium` | `parse` | 29.560 µs | 20.138 µs | 1.47x |
| `mindmap_medium` | `layout` | 138.47 µs | 63.060 µs | 2.20x |
| `mindmap_medium` | `render` | 82.378 µs | 68.264 µs | 1.21x |
| `mindmap_medium` | `end_to_end` | 295.39 µs | 158.66 µs | 1.86x |
| `architecture_medium` | `parse` | 4.3736 µs | 7.1295 µs | 0.61x |
| `architecture_medium` | `layout` | 117.16 µs | 12.217 µs | 9.59x |
| `architecture_medium` | `render` | 51.908 µs | 18.902 µs | 2.75x |
| `architecture_medium` | `end_to_end` | 189.86 µs | 41.538 µs | 4.57x |
| `c4_medium` | `parse` | 203.04 µs | 20.666 µs | 9.82x |
| `c4_medium` | `layout` | 62.368 µs | 75.483 µs | 0.83x |
| `c4_medium` | `render` | 79.172 µs | 51.848 µs | 1.53x |
| `c4_medium` | `end_to_end` | 308.13 µs | 138.41 µs | 2.23x |

## Summary (geometric mean of ratios)

- `parse`: `2.07x`
- `layout`: `2.59x`
- `render`: `1.72x`
- `end_to_end`: `2.67x`
