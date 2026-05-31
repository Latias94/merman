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
- 2026-05-31: M15-070 class hierarchical namespaces complete.
  - Upstream check: Mermaid 11.15 class adds hierarchical namespace rendering for dotted names and syntactic nesting; `class.hierarchicalNamespaces` defaults to `true`, while `false` restores the <=11.14 flat dotted namespace behavior. Namespace bodies accept nested namespaces and notes, and namespace notes are parented to the active namespace.
  - `cargo nextest run -p merman-core parse_diagram_class_hierarchical_dotted_namespace_and_notes parse_diagram_class_nested_namespace_syntax_builds_qualified_parents parse_class_exposes_11_15_hierarchical_namespaces_default_and_override` failed before implementation: `class.hierarchicalNamespaces` was missing from defaults, namespace notes failed to parse, and nested namespace syntax failed to parse.
  - `cargo nextest run -p merman-render class_layout_dotted_namespace_builds_hierarchical_clusters class_layout_namespace_note_stays_inside_namespace_cluster class_svg_dotted_namespace_titles_use_hierarchical_segment_labels` failed before implementation: dotted namespaces were rendered as one flat cluster, namespace note parsing failed, and SVG output lacked the parent namespace cluster.
  - `cargo nextest run -p merman-core parse_diagram_class_hierarchical_dotted_namespace_and_notes parse_diagram_class_nested_namespace_syntax_builds_qualified_parents parse_diagram_class_hierarchical_namespaces_can_be_disabled parse_class_exposes_11_15_hierarchical_namespaces_default_and_override` passed: 4 tests.
  - `cargo nextest run -p merman-render class_layout_dotted_namespace_builds_hierarchical_clusters class_layout_namespace_note_stays_inside_namespace_cluster class_layout_hierarchical_namespaces_false_keeps_flat_dotted_cluster class_svg_dotted_namespace_titles_use_hierarchical_segment_labels` passed: 4 tests.
  - `cargo nextest run -p merman-core class` passed: 44 tests.
  - `cargo nextest run -p merman-render class` passed: 27 tests.
  - `cargo run -p xtask -- update-snapshots` refreshed class semantic goldens; non-class reserialization noise was discarded.
  - `cargo run -p xtask -- update-layout-snapshots --diagram class` refreshed class layout goldens for hierarchical namespaces.
  - `cargo nextest run -p merman-core` passed: 536 tests.
  - `cargo nextest run -p merman-render` passed: 244 tests.
  - `cargo fmt --check` passed.
  - `git diff --check` passed with only LF-to-CRLF warnings for `crates/merman-core/src/diagrams/class_grammar.lalrpop` and `docs/workstreams/mermaid-11-15-baseline-upgrade/CONTEXT.jsonl`.
- 2026-05-31: M15-080 scoped internal SVG marker IDs complete.
  - Upstream check: Mermaid 11.14 PR #7526 prefixes internal SVG element IDs with the diagram SVG element ID across diagram types; exact marker selectors should move to suffix-compatible selectors such as `[id$="-arrowhead"]`.
  - `cargo nextest run -p merman-render c4_marker_ids_are_prefixed_with_diagram_svg_id journey_marker_ids_are_prefixed_with_diagram_svg_id timeline_marker_ids_are_prefixed_with_diagram_svg_id sequence_marker_ids_are_prefixed_with_diagram_svg_id_and_css_uses_suffix_selectors` failed before implementation: c4, journey, timeline, and sequence still emitted bare `id="arrowhead"` / `url(#arrowhead)` markers, and sequence CSS used exact `#arrowhead` / `#sequencenumber` selectors.
  - `cargo nextest run -p merman-render c4_marker_ids_are_prefixed_with_diagram_svg_id journey_marker_ids_are_prefixed_with_diagram_svg_id timeline_marker_ids_are_prefixed_with_diagram_svg_id sequence_marker_ids_are_prefixed_with_diagram_svg_id_and_css_uses_suffix_selectors` passed: 4 tests.
  - `cargo nextest run -p merman-render sequence` passed: 16 tests.
  - `cargo nextest run -p merman-render` passed: 248 tests.
  - `cargo fmt` applied standard formatting after the first `cargo fmt --check` reported two line-wrap diffs.
  - `cargo fmt --check` passed.
  - `git diff --check` passed with only the LF-to-CRLF warning for `docs/workstreams/mermaid-11-15-baseline-upgrade/CONTEXT.jsonl`.
