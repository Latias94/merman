# M15RV-089 - Architecture Residual Classification Refresh

Task: M15RV-089

## Summary

The Architecture root-residual queue in this lane was stale. Later HPD-050 work closed additional
rows, including the isolated top-level service root-bounds case, so the fresh Architecture
`parity-root` report now has `25` mismatches instead of the older `29`.

This slice only updates workstream evidence and handoff state. It does not change renderer code.

## Evidence

- Structural Architecture `parity` report:
  `target/compare/architecture_report_parity_hpd050_residual_classification_refresh.md`, passed.
- Root Architecture `parity-root` report:
  `target/compare/architecture_report_parity_root_hpd050_residual_classification_refresh.md`,
  expected failure with `25` dom mismatches.

## Queue Correction

Do not continue M15RV-089 from the old `batch4_init_small_icons` tail. The fresh report no longer
contains:

- `stress_architecture_batch4_init_small_icons_061`
- `stress_architecture_batch4_init_fontsize_wrap_063`
- `stress_architecture_edge_label_corner_cases_012`
- `stress_architecture_fan_in_out_021`
- `stress_architecture_deep_nesting_013`
- `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`
- `stress_architecture_disconnected_islands_046`

The remaining larger Architecture rows are:

- `stress_architecture_junction_fork_join_026`
- `stress_architecture_batch5_long_titles_and_punct_076`
- `stress_architecture_html_titles_and_escapes_041`
- `stress_architecture_unicode_and_xml_escapes_019`
- `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`
- `stress_architecture_nested_groups_002`
- `stress_architecture_group_port_edges_017`

Keep the smaller icon/default/reasonable-height rows classified as diagnostic browser/Cytoscape
bbox lattice unless a reusable source rule or generated metric path appears.
