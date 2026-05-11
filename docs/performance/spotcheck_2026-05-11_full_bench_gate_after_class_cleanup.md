# Full Bench Gate After Class Text Lookup Cleanup - 2026-05-11

Command:

```powershell
cargo bench -p merman --features render
```

Result: passed.

Notes:

- A first run with a 20 minute timeout window expired before completion.
- The same command completed successfully with a 1 hour timeout window.
- Criterion reported a mix of improvements and regressions against the saved local baselines. The
  recent code changes were narrow Class text lookup pruning plus documentation, so treat this run as
  release-gate evidence rather than a causal performance regression report.
- `gnuplot` was not available; Criterion used the plotters backend.

Representative current estimates from `target/criterion/*/new/estimates.json`:

| benchmark | mean 95% CI |
| --- | ---: |
| `parse/class_tiny` | 4.687-6.018 us |
| `parse/class_medium` | 125.917-132.157 us |
| `parse/class_namespace_dense` | 76.237-79.102 us |
| `layout/class_medium` | 943.769-954.728 us |
| `layout/class_namespace_dense` | 1.027-1.058 ms |
| `render/class_medium` | 552.468-581.983 us |
| `render/class_namespace_dense` | 213.912-226.294 us |
| `end_to_end/class_medium` | 1.826-1.907 ms |
| `end_to_end/class_namespace_dense` | 1.520-1.606 ms |
| `layout_stress/architecture_reasonable_height_layout_x50` | 44.112-46.607 ms |
| `layout_stress/mindmap_balanced_tree_layout_x50` | 10.419-11.525 ms |
| `render_stress/flowchart_medium_x50` | 24.325-26.400 ms |
| `text_measure_stress/computed_length_plain/node_label` | 0.955-1.001 us |
| `text_measure_stress/wrapped_svg_like_plain/wrapped_cluster_title` | 15.549-16.335 us |
