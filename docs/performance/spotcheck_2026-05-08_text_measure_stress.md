# Text Measure Stress Spotcheck

This report captures a same-machine Criterion spotcheck for the focused text measurement benchmark
added before future cache work. The numbers are local regression anchors, not release performance
guarantees.

## Parameters

- Date: 2026-05-08
- Git commit: `a0df717b`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Raw output: `target/bench/text_measure_stress_2026-05-08.txt`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Note: `text_measure_stress` sets `sample_size(50)` internally, so Criterion collected 50 samples
  even though the command line requested 20.

## Command

```text
cargo bench -p merman --features render --bench text_measure_stress -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Mid Estimates

| case | computed plain | wrapped plain | computed bold | wrapped bold |
| --- | ---: | ---: | ---: | ---: |
| `node_label` | 1.2902 us | 2.4747 us | 1.3613 us | 2.2545 us |
| `edge_label` | 1.0981 us | 2.8568 us | 1.5201 us | 2.5212 us |
| `subgraph_title` | 1.2480 us | 2.8466 us | 1.2512 us | 2.9324 us |
| `wrapped_cluster_title` | 2.6946 us | 17.195 us | 2.4888 us | 18.097 us |
| `special_characters` | 1.3595 us | 4.3265 us | 1.4159 us | 4.6575 us |

## Notes

- Wrapped long cluster titles are the highest-cost target in this focused set and should be the
  primary same-machine regression anchor for text measurement cache changes.
- Plain and bold computed-length probes stay in the low microsecond range for short labels.
- `Gnuplot not found, using plotters backend` was reported and does not affect the timing estimates.
