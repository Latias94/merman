# Full Bench Gate After Flowchart Override Inventory Cleanup - 2026-05-14

Command:

```powershell
cargo bench -p merman --features render
```

Result: passed.

Scope:

- Package-level `merman` benches with the `render` feature.
- Pipeline parse/layout/render/end-to-end benches.
- Flowchart, architecture, and mindmap stress benches.
- `text_measure_stress`.

Notes:

- Wall time was approximately `51m 13s`.
- `gnuplot` was not available, so Criterion used the plotters backend.
- This run followed a table-only Flowchart root override inventory cleanup. Treat it as
  release-gate evidence that the benchmark suite still completes, not as causal performance
  attribution.

Representative current estimates from `target/criterion/*/new/estimates.json`:

| benchmark | mean 95% CI |
| --- | ---: |
| `layout_stress/architecture_reasonable_height_layout_x50` | 47.063-49.017 ms |
| `render_stress/architecture_many_services_one_group_x200` | 55.142-57.367 ms |
| `render_stress/flowchart_medium_x50` | 25.041-26.430 ms |
| `render_stress/flowchart_ports_heavy_x20` | 6.203-6.497 ms |
| `layout_stress/mindmap_balanced_tree_layout_x50` | 9.594-10.053 ms |
| `parse/flowchart_medium` | 301.847-314.332 us |
| `layout/flowchart_medium` | 8.182-8.614 ms |
| `render/flowchart_medium` | 533.175-561.951 us |
| `end_to_end/flowchart_medium` | 18.283-22.824 ms |
| `parse/sequence_medium` | 41.259-44.427 us |
| `layout/sequence_medium` | 297.717-310.850 us |
| `render/sequence_medium` | 53.001-55.771 us |
| `end_to_end/sequence_medium` | 402.296-428.720 us |
| `parse/class_medium` | 114.586-120.222 us |
| `layout/class_medium` | 1.572-1.664 ms |
| `render/class_medium` | 637.165-679.898 us |
| `end_to_end/class_medium` | 2.123-2.237 ms |
| `text_measure_stress/computed_length_plain/node_label` | 1.835-1.998 us |
| `text_measure_stress/wrapped_svg_like_plain/wrapped_cluster_title` | 26.593-27.859 us |
