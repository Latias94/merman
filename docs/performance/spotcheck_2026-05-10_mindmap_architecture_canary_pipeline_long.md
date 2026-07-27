# Mindmap/Architecture Canary Pipeline Long Spot-check

This note records a longer same-machine Criterion canary run after the C4 override cleanup. It is
the more reliable validation pass for the local Mindmap/Architecture checkpoint.

## Parameters

- Date: 2026-05-10
- Git commit: `ecba84ea`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 30 --warm-up-time 2 --measurement-time 3 "architecture_medium|mindmap_medium"`

## Results

Times are Criterion mid estimates.

| benchmark | observed time range | local change range |
| --- | ---: | ---: |
| `parse/mindmap_medium` | 25.680-27.679 us | +1.18%..+14.70% |
| `parse_known_type/mindmap_medium` | 285.28-311.72 us | +4.66%..+24.01% |
| `layout/mindmap_medium` | 149.17-160.93 us | -56.73%..-52.57% |
| `render/mindmap_medium` | 87.724-94.730 us | +3.66%..+14.55% |
| `end_to_end/mindmap_medium` | 271.05-293.51 us | -14.57%..-3.92% |
| `parse/architecture_medium` | 4.5578-4.9821 us | +4.99%..+19.09% |
| `parse_known_type/architecture_medium` | 151.66-166.63 us | -4.95%..+4.68% |
| `layout/architecture_medium` | 112.91-123.05 us | -32.11%..-20.63% |
| `render/architecture_medium` | 55.040-60.238 us | +3.49%..+19.58% |
| `end_to_end/architecture_medium` | 189.89-206.62 us | +2.48%..+18.47% |

## Interpretation

- `layout/mindmap_medium` and `layout/architecture_medium` both keep the strong local improvement
  signal.
- `end_to_end/mindmap_medium` also improved in the longer run.
- `parse/mindmap_medium` is still noisy but not enough to justify parser work yet.
- `render/*` and `end_to_end/architecture_medium` remain mixed in this run, so they should stay in
  the follow-up queue rather than becoming the next immediate cleanup target.

## Follow-up

- Keep the short spotcheck as the quick triage note, but use this longer run as the default local
  evidence for the current checkpoint.
- Re-run the cross-repo stage spotcheck only when the external `mermaid-rs-renderer` checkout is
  on a toolchain that can build `json5` cleanly.
