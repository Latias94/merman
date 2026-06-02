# HPD-050 - Architecture Residual Classification Refresh

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Fresh Architecture family reports were used to reconcile the HPD-050 evidence with the older
M15RV-089 root-residual handoff. The active Architecture root queue is now `25` mismatches after
the isolated top-level service root-bounds seam. The previous `29`-row queue and its
`batch4_init_small_icons` next-step note are stale.

No renderer behavior changed in this slice.

## Fresh Evidence

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_hpd050_residual_classification_refresh.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_residual_classification_refresh.md`
  remained an expected failure with `25` dom mismatches.

## Classification

Rows that should not be reopened unless a fresh report regresses:

- `stress_architecture_batch4_init_small_icons_061`
- `stress_architecture_batch4_init_fontsize_wrap_063`
- `stress_architecture_edge_label_corner_cases_012`
- `stress_architecture_fan_in_out_021`
- `stress_architecture_deep_nesting_013`
- `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`
- `stress_architecture_disconnected_islands_046`

Remaining larger audit queue:

- `stress_architecture_junction_fork_join_026`: `+13.976px`, source-input matched but still split
  by CLI/browser-probe and solver/phase behavior.
- `stress_architecture_batch5_long_titles_and_punct_076` and
  `stress_architecture_html_titles_and_escapes_041`: `+5px` group/service Cytoscape bbox tails.
- `stress_architecture_unicode_and_xml_escapes_019`: `+3px` group/service bbox tail.
- `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`: `-2.5px` custom-init
  canvas-label / compound-bounds tail; the tested browser-bbox adjustment was rejected.
- `stress_architecture_nested_groups_002`: `+2.5px` nested-compound/FCoSE residual.
- `stress_architecture_group_port_edges_017`: `+1.468px` source-input-matched
  manatee-vs-Cytoscape-FCoSE compound-bound drift.

## Next Action

Continue HPD-050 only with source-backed or generated-measurement evidence. Do not add root pins,
one-off text constants, or broad FCoSE rewrites just to reduce the count.
