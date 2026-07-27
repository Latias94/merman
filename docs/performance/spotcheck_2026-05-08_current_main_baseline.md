# Current Main Performance Baseline

This report captures a current-main local Criterion baseline for the fearless-refactor workstream.
The numbers are same-machine regression anchors, not release performance guarantees.

## Parameters

- Date: 2026-05-08
- Git commit: `523e39dd`
- Rust: `rustc 1.87.0 (17067e9ac 2025-05-09)`, host `x86_64-pc-windows-msvc`
- Raw output: `target/bench/current_main_2026-05-08.txt`
- Criterion options: `--noplot --sample-size 20 --warm-up-time 1 --measurement-time 1`
- Note: the stress benches set `sample_size(50)` internally, so they collected 50 samples even
  when the command line requested 20.

Discarded command:

```text
cargo bench -p merman --features render -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

That package-wide command forwards Criterion options to the lib bench harness first and fails with
`Unrecognized option: 'noplot'`. The effective baseline used explicit `--bench` commands instead:

```text
cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench flowchart_stress -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench architecture_stress -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench architecture_layout_stress -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
cargo bench -p merman --features render --bench mindmap_layout_stress -- --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

## Pipeline Mid Estimates

| fixture | parse | parse_known_type | layout | render | end_to_end |
| --- | ---: | ---: | ---: | ---: | ---: |
| `flowchart_medium` | 330.53 us | 690.80 us | 5.6492 ms | 508.80 us | 6.4616 ms |
| `class_medium` | 96.776 us | 160.70 us | 1.2285 ms | 608.93 us | 1.8349 ms |
| `sequence_tiny` | 2.1293 us | 5.9351 us | 8.7990 us | 18.429 us | 41.163 us |
| `sequence_medium` | 38.013 us | 71.666 us | 98.312 us | 61.332 us | 210.52 us |
| `mindmap_medium` | 23.484 us | 249.86 us | 231.31 us | 94.100 us | 352.34 us |
| `architecture_medium` | 4.4372 us | 146.60 us | 111.12 us | 47.923 us | 184.06 us |

## Stress Mid Estimates

| bench | batch time | approximate per operation |
| --- | ---: | ---: |
| `render_stress/flowchart_medium_x50` | 21.596 ms | 431.92 us/render |
| `render_stress/flowchart_ports_heavy_x20` | 5.5594 ms | 277.97 us/render |
| `render_stress/architecture_many_services_one_group_x200` | 50.532 ms | 252.66 us/render |
| `layout_stress/architecture_reasonable_height_layout_x50` | 44.822 ms | 896.44 us/layout |
| `layout_stress/mindmap_balanced_tree_layout_x50` | 9.3970 ms | 187.94 us/layout |

## Notes

- `treemap_medium` and `xychart_medium` are skipped by the pipeline bench pre-checks because the
  current bench fixtures do not successfully parse/render through that bench path.
- Criterion printed change/regression notes for benches that had local historical samples. Treat
  the mid estimates above as the current baseline; use future same-machine runs for comparisons.
- `Gnuplot not found, using plotters backend` was reported and does not affect the timing estimates.
