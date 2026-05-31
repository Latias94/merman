# Mermaid 11.15 Baseline Upgrade - Evidence And Gates

Status: Active
Last updated: 2026-05-31

## Smallest Current Repro

```bash
cargo nextest run -p merman-core sequence
```

This validates the first planned slice: sequence parser and semantic model compatibility.

## Gate Set

### Targeted Iteration Gates

```bash
cargo nextest run -p merman-core sequence
cargo nextest run -p merman-render sequence
cargo nextest run -p merman-render flowchart
cargo nextest run -p merman-render sankey
cargo nextest run -p merman-render xychart
```

Use the narrowest package/test filter that covers the active task.

### Formatting Gate

```bash
cargo fmt --check
```

### Package Gates

```bash
cargo nextest run -p merman-core
cargo nextest run -p merman-render
```

### Broader Closeout Gate

```bash
cargo nextest run --workspace
```

If the workspace gate is too slow for a slice, record the narrower command and why it is sufficient
for that task.

### Review Gate

Run `review-workstream` before accepting task or lane completion. Fresh verification is required
before marking a task, Codex goal, or lane complete.

## Evidence Anchors

- `repo-ref/mermaid/packages/mermaid/CHANGELOG.md`
- `docs/workstreams/mermaid-11-15-baseline-upgrade/DESIGN.md`
- `docs/workstreams/mermaid-11-15-baseline-upgrade/TODO.md`
- task-specific tests and fixture updates

## Evidence Log

- 2026-05-31: Opened the workstream from the 11.13-11.15 audit. No code gates run yet.
- 2026-05-31: M15-020 sequence decimal `autonumber` complete.
  - `cargo nextest run -p merman-core parse_diagram_sequence_autonumber_allows_decimal_start_and_step` failed before implementation with `UnrecognizedToken` for `.1`.
  - `cargo nextest run -p merman-core parse_diagram_sequence_autonumber` passed: 2 tests.
  - `cargo nextest run -p merman-core sequence` passed: 31 tests.
  - `cargo nextest run -p merman-render sequence_autonumber_renders_decimal_sequence_numbers` passed: 1 test.
  - `cargo nextest run -p merman-render sequence` passed: 15 tests.
  - `cargo nextest run -p merman-core` passed: 528 tests.
  - `cargo fmt --check` passed.
- 2026-05-31: M15-030 flowchart `datastore` shape support complete.
  - Upstream check: `datastore` / `data-store` are distinct from `bow-rect` / `stored-data` and render as a rect with `stroke-dasharray=width height`.
  - `cargo nextest run -p merman-core parse_diagram_flowchart_node_data_shape_data_accepts_datastore` failed before implementation with `No such shape: datastore.`.
  - `cargo nextest run -p merman-core parse_diagram_flowchart_node_data_shape_data_accepts_datastore` passed: 1 test.
  - `cargo nextest run -p merman-render flowchart_datastore_shape_renders_top_and_bottom_border_rect` passed: 1 test.
  - `cargo nextest run -p merman-core flowchart` passed: 94 tests.
  - `cargo nextest run -p merman-render flowchart` passed: 73 tests.
  - `cargo fmt --check` passed.
- 2026-05-31: M15-031 flowchart default curve change complete.
  - Upstream check: Mermaid 11.13 changes default flowchart curve from `basis` to `rounded`, while explicit `flowchart.curve: "basis"` restores the previous smooth curve.
  - `cargo nextest run -p merman-render flowchart_default_curve_renders_rounded_edges_while_basis_remains_available` failed before implementation because the default edge path still used `curveBasis` `C` commands.
  - `cargo nextest run -p merman-render flowchart_default_curve_renders_rounded_edges_while_basis_remains_available` passed: 1 test.
  - `cargo nextest run -p merman-render flowchart` passed: 74 tests.
  - `cargo nextest run -p merman-core flowchart` passed: 94 tests.
  - `cargo nextest run -p merman-core config` passed: 9 tests.
  - `cargo nextest run -p merman-core` passed: 529 tests.
  - `cargo nextest run -p merman-render` passed: 231 tests.
  - `cargo fmt --check` passed.
- 2026-05-31: M15-040 Architecture FCoSE config exposure complete.
  - Upstream check: Mermaid 11.15 Architecture defaults include `randomize=false`, `nodeSeparation=75`, `idealEdgeLengthMultiplier=1.5`, `edgeElasticity=0.45`, `numIter=2500`, and `seed=1`.
  - `cargo nextest run -p merman-core parse_architecture_exposes_11_15_fcose_config_defaults_and_overrides` failed before implementation because `architecture.randomize` was missing from generated defaults.
  - `cargo nextest run -p merman-render architecture_ideal_edge_length_multiplier_changes_same_group_spacing` failed before implementation because layout still hardcoded the 1.5 multiplier.
  - `cargo nextest run -p merman-core parse_architecture_exposes_11_15_fcose_config_defaults_and_overrides` passed: 1 test.
  - `cargo nextest run -p merman-render architecture_ideal_edge_length_multiplier_changes_same_group_spacing` passed: 1 test.
  - `cargo nextest run -p merman-render architecture_randomize_and_node_separation_change_layout` passed: 1 test.
  - `cargo nextest run -p merman-render architecture_edge_elasticity_changes_same_group_layout` passed: 1 test.
  - `cargo nextest run -p merman-render architecture_num_iter_changes_layout_budget` passed: 1 test.
  - `cargo nextest run -p manatee` passed: 11 tests.
  - `cargo nextest run -p merman-core architecture` passed: 10 tests.
  - `cargo nextest run -p merman-core config` passed: 10 tests.
  - `cargo nextest run -p merman-render architecture` passed: 12 tests.
  - `cargo nextest run -p merman-core` passed: 530 tests.
  - `cargo nextest run -p merman-render` passed: 236 tests.
  - `cargo fmt --check` passed.
- 2026-05-31: M15-050 Sankey 11.15 config support complete.
  - Upstream check: Mermaid 11.15 Sankey defaults include `nodeWidth=10`, `nodePadding=12`, `labelStyle=legacy`, and `nodeColors` as an optional ID-to-color map; renderer adds 15px to node padding when `showValues=true`, uses custom node colors for nodes and link colors, and renders `outlined` labels as background/foreground text.
  - `cargo nextest run -p merman-render sankey_layout_uses_configured_node_width_and_padding` failed before implementation because layout still hardcoded `nodeWidth=10`.
  - `cargo nextest run -p merman-render sankey_svg_uses_configured_node_colors_and_outlined_labels` failed before implementation because SVG still used Tableau colors and legacy single labels.
  - `cargo nextest run -p merman-core parse_sankey_exposes_11_15_config_defaults_and_overrides` failed before implementation because `sankey.nodeWidth` was missing from generated defaults.
  - `cargo nextest run -p merman-render sankey_layout_uses_configured_node_width_and_padding sankey_node_geometry_constants_match_mermaid sankey_layout_uses_mermaid_node_geometry` passed: 3 tests.
  - `cargo nextest run -p merman-render sankey_svg_uses_configured_node_colors_and_outlined_labels` passed: 1 test.
  - `cargo nextest run -p merman-core parse_sankey_exposes_11_15_config_defaults_and_overrides` passed: 1 test.
  - `cargo nextest run -p merman-render sankey` passed: 4 tests.
  - `cargo nextest run -p merman-core sankey` passed: 5 tests.
  - `cargo nextest run -p merman-core config` passed: 11 tests.
  - `cargo run -p xtask -- update-layout-snapshots --diagram sankey` refreshed Sankey layout goldens for the 11.15 padding baseline.
  - `cargo nextest run -p merman-core` passed: 531 tests.
  - `cargo nextest run -p merman-render` passed: 238 tests.
  - `cargo fmt --check` passed.
- 2026-05-31: M15-060 xyChart data-label config support complete.
  - Upstream check: Mermaid 11.15 xyChart schema defaults `showDataLabelOutsideBar=false`; the renderer uses it to place horizontal bar labels outside with `text-anchor=start` and vertical bar labels above the bar with `dominant-baseline=auto`, and fills data labels from theme `dataLabelColor`.
  - `cargo nextest run -p merman-render xychart_vertical_bar_data_label_can_render_outside_with_configured_color` failed before implementation because vertical labels still used `dominant-baseline="hanging"` and `fill="black"`.
  - `cargo nextest run -p merman-core parse_xychart_exposes_11_15_data_label_outside_default_and_override` failed before implementation because `xyChart.showDataLabelOutsideBar` was missing from generated defaults.
  - `cargo nextest run -p merman-render xychart_vertical_bar_data_label_can_render_outside_with_configured_color` passed: 1 test.
  - `cargo nextest run -p merman-render xychart_horizontal_bar_data_label_can_render_outside xychart_vertical_bar_data_label_can_render_outside_with_configured_color` passed: 2 tests.
  - `cargo nextest run -p merman-core parse_xychart_exposes_11_15_data_label_outside_default_and_override` passed: 1 test.
  - `cargo nextest run -p merman-render xychart` passed: 2 tests.
  - `cargo nextest run -p merman-core xychart` passed: 17 tests.
  - `cargo nextest run -p merman-core config` passed: 11 tests.
  - `cargo nextest run -p merman-core` passed: 532 tests.
  - `cargo nextest run -p merman-render` passed: 240 tests.
  - `cargo fmt --check` passed.
