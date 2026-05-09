# Mindmap/Architecture Canary Pipeline Spot-check

This note records a same-machine Criterion canary run for the current Mindmap and Architecture
performance gaps. The run is intentionally short; use it as local regression evidence and stage
direction, not as a release-grade benchmark.

## Parameters

- Date: 2026-05-10
- Git commit: `ecba84ea`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Bench command:
  `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1 "architecture_medium|mindmap_medium"`
- Criterion comparison source: existing local history in `target/criterion`

## Results

Times are Criterion mean estimate confidence intervals.

| benchmark | observed time range | local change range |
| --- | ---: | ---: |
| `parse/mindmap_medium` | 24.201-26.082 us | +1.38%..+10.99% |
| `parse_known_type/mindmap_medium` | 240.732-280.187 us | -1.42%..+15.70% |
| `layout/mindmap_medium` | 147.009-158.703 us | -40.25%..-33.02% |
| `render/mindmap_medium` | 79.950-85.865 us | -14.36%..-4.24% |
| `end_to_end/mindmap_medium` | 286.573-318.315 us | -18.17%..-6.93% |
| `parse/architecture_medium` | 4.312-4.753 us | -9.28%..+7.24% |
| `parse_known_type/architecture_medium` | 149.858-161.034 us | -8.61%..+12.31% |
| `layout/architecture_medium` | 153.782-174.813 us | -46.18%..-38.06% |
| `render/architecture_medium` | 47.816-52.549 us | -10.13%..+4.65% |
| `end_to_end/architecture_medium` | 170.775-188.263 us | -40.80%..-33.81% |

## Interpretation

- The strong local signal is layout-stage improvement for both current canaries.
- `mindmap_medium` also shows a render-stage improvement and a smaller end-to-end win.
- `architecture_medium` end-to-end movement is dominated by the layout-stage change; render is
  within noise in this short run.
- The `parse/mindmap_medium` regression band is small enough to treat as a follow-up validation
  point, not as evidence to prioritize parser work yet.

## Follow-up

- Re-run a longer canary before claiming release numbers.
- Use `MERMAN_MINDMAP_LAYOUT_TIMING=1` and `MANATEE_COSE_TIMING=1` if the next Mindmap pass targets
  the layout internals.
- Re-run the merman-vs-mmdr stage spotcheck after another Architecture layout cleanup to confirm
  whether the cross-repo gap moved, because this run only compares against local Criterion history.
