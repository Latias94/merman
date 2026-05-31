# Flowchart 11.15 SVG Convergence - Evidence And Gates

Status: Active
Last updated: 2026-06-01

## Smallest Current Repro

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
```

This currently fails against the fresh Mermaid 11.15 Flowchart target with 67 DOM mismatches plus
one unsupported `flowchart-elk` local layout failure.

## Gate Set

### Fresh Target Generation

```bash
cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out target/upstream-svgs-11-15-flowchart
```

Use this before trusting stored Flowchart SVG baselines. The target directory is a generated
evidence artifact, not a committed source of truth.

### Targeted Iteration Gate

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --filter <fixture-filter> --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
```

Every renderer slice should name representative filters from the category being fixed.

### Full Fresh Flowchart Gate

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
```

This is authoritative for renderer convergence before stored baseline refresh.

### Stored Baseline Gate

```bash
cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out fixtures/upstream-svgs
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
```

Run only after the fresh Flowchart gate is green or after documented skips are in place.

### Package And Diff Gates

```bash
cargo nextest run -p merman-render flowchart
cargo fmt --check
git diff --check
```

## Evidence Log

- 2026-06-01 M15C-060 Flowchart triage:
  - `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --filter upstream_docs_math_flowcharts_001 --out target/upstream-svgs-11-15-flowchart-probe`:
    passed.
  - Fresh Mermaid 11.15 and local output both include MathML `columnalign` for
    `upstream_docs_math_flowcharts_001`; the old stored baseline did not. The stored Math fixture
    was refreshed as part of the umbrella M15C-060 triage.
  - Initial Flowchart 11.15 DOM-envelope renderer changes made
    `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_math_flowcharts_001`
    pass for the targeted stored Math fixture.
  - `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out target/upstream-svgs-11-15-flowchart`:
    generated 1070 fresh Mermaid 11.15 Flowchart SVGs after the shell timeout expired and the
    original `xtask` process continued. Five parser-only or upstream-render-failing fixtures did
    not produce SVGs:
    `upstream_flow_text_ellipse_vertex_parser_only_spec`,
    `upstream_html_demos_flowchart_flowchart_040_parser_only_katex`,
    `upstream_html_demos_flowchart_flowchart_042_parser_only_katex`,
    `upstream_html_demos_flowchart_flowchart_044_parser_only_katex`, and
    `upstream_html_demos_flowchart_graph_039_parser_only_katex`.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 594 canonical XML mismatches plus one local layout failure for
    `flowchart/upstream_html_demos_flowchart_elk_flowchart_elk_001`.
  - Lightweight classification from representative fresh diffs:
    `outer_path_class=203`, `edge_markdown_rows=61`, `missing_row_class=61`,
    `shape_path_class=77`, `anchor_or_click=23`, `html_foreign_object=556`,
    `subgraph_cluster=594`, `other=0`.
  - Representative observed deltas:
    `probe_flowchart_edge_markdown_html_false_982` needs Mermaid 11.15 markdown row tspan
    structure; `stress_flowchart_classdef_and_inline_classes_003` and
    `stress_flowchart_clicks_and_tooltips_005` expose missing `outer-path` shape classes.
- 2026-06-01 F115-020/F115-030 first Flowchart 11.15 convergence slice:
  - Implemented Flowchart 11.15 DOM-envelope alignment for drop-shadow defs, margin markers,
    `data-look`, scoped node/edge ids, classic rounded-rect output, cluster ids, and first-order
    `outer-path` class surfaces.
  - Removed the stale pre-11.15 assumption that bare backtick-wrapped pipe edge labels render as
    empty SVG text. Mermaid 11.15 preserves those labels as plain text.
  - Added Mermaid 11.15 SVG-label row semantics (`row text-outer-tspan`) and centered edge-label
    `text-anchor` attributes.
  - Updated Flowchart `htmlLabels` precedence to Mermaid 11.15 behavior: root `htmlLabels` first,
    `flowchart.htmlLabels` as deprecated fallback.
  - Targeted fresh `compare-svg-xml` filters passed for
    `upstream_docs_math_flowcharts_001`,
    `stress_flowchart_classdef_and_inline_classes_003`,
    `stress_flowchart_clicks_and_tooltips_005`,
    `probe_flowchart_edge_markdown_html_false_982`,
    `probe_flowchart_edge_quoted_markdown_html_false_985`,
    `stress_flowchart_cluster_minimal_title_placeholder_024`,
    `stress_flowchart_cluster_dense_children_021`,
    `stress_flowchart_html_labels_global_false_flowchart_true_069`,
    `stress_flowchart_html_labels_global_false_flowchart_unset_071`, and
    `stress_flowchart_html_labels_global_true_flowchart_false_070`, all using
    `--upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 359 canonical XML mismatches plus the existing `flowchart-elk` local layout
    failure. This is a reduction from the initial 594 fresh mismatches.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - First `cargo nextest run -p merman-render flowchart` attempt failed during compilation with a
    transient Windows/cache error: `crate palette required to be available in rlib format`.
  - Re-run `cargo nextest run -p merman-render flowchart`: passed, 74 tests.
- 2026-06-01 F115-040/F115-050 shapeData label and hexagon slice:
  - Aligned Mermaid 11.15 `shapeData` label semantics: `label` defaults to
    `labelType=markdown`, while explicit `labelType: text|string|markdown` is honored.
  - Aligned Flowchart node HTML-label semantics with Mermaid 11.15 `labelHelper`: normal node
    labels read root `htmlLabels` directly and default to HTML labels when root is unset, while
    edge and cluster labels still use `getEffectiveHtmlLabels(...)` and honor deprecated
    `flowchart.htmlLabels`.
  - Added `markdown-node-label` class coverage to Flowchart node HTML label spans, including
    icon/image label renderers.
  - Aligned classic hexagon rendering with Mermaid 11.15's 6-point polygon container and restricted
    the RoughJS path branch to `look=handDrawn`.
  - Targeted fresh `compare-svg-xml` filters passed for
    `stress_flowchart_label_br_list_063`,
    `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset1_001`,
    `stress_flowchart_subgraph_markdown_titles_013`, and
    `upstream_cypress_flowchart_icon_spec_should_render_aws_icons_with_labels_and_rect_elements_005`,
    all using `--upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`.
  - `cargo nextest run -p merman-core parse_diagram_flowchart_node_data_label_type_defaults_to_markdown_but_can_be_overridden parse_diagram_flowchart_node_data_shape_data_accepts_datastore parse_diagram_flowchart_node_data_multiple_properties_same_line`:
    passed, 3 tests.
  - `cargo nextest run -p merman-render flowchart_node_labels_use_root_html_labels_when_flowchart_html_labels_is_false flowchart_classic_hexagon_renders_polygon_container`:
    passed, 2 tests.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 143 canonical XML mismatches plus the existing `flowchart-elk` local layout
    failure. This is a reduction from 359 fresh mismatches after F115-020/F115-030.
  - Remaining mismatch classification is now dominated by shape matrix fixtures (heuristic name
    count: 94), polygon point-model deltas (18), config/theme fixtures (17), and smaller
    image/icon, cluster, and edge groups. The previous dominant missing markdown node-label class
    category is reduced to one fresh mismatch.
  - `cargo nextest run -p merman-core flowchart`: passed, 95 tests.
  - `cargo nextest run -p merman-render flowchart`: passed, 76 tests.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
- 2026-06-01 F115-040 no-label outer-path slice:
  - Added Mermaid 11.15 `outer-path` group classes for no-label special shapes where upstream
    emits them: `stop`/`framed-circle`, `bolt`/`lightning-bolt`, and
    `crossed-circle`/`summary`. `filled-circle` intentionally remains a bare group to match
    upstream.
  - Targeted fresh `compare-svg-xml` filters passed for
    `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset16_016`,
    `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset19_019`,
    `upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037`,
    `upstream_cypress_newshapes_spec_newshapessets_newshapesset2_tb_nolabel_009`,
    `upstream_cypress_newshapes_spec_newshapessets_newshapesset3_tb_nolabel_017`, and
    `upstream_cypress_oldshapes_spec_shapessets_shapesset2_tb_nolabel_009`.
  - `cargo nextest run -p merman-render flowchart_no_label_special_shapes_render_outer_path_group`:
    passed, 1 test.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 95 canonical XML mismatches plus the existing `flowchart-elk` local layout failure.
    This is a reduction from 143 fresh mismatches before the slice.
  - Remaining mismatch classification after this slice: `missingOuter=0`, shape matrix fixture name
    count 49, polygon point-model deltas 18, config/theme fixtures 17, image/icon 5, cluster 7,
    edge 2, and one residual missing markdown node-label class.
  - `cargo nextest run -p merman-render flowchart`: passed, 77 tests.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
- 2026-06-01 F115-040/F115-050 theme-gradient slice:
  - Aligned Mermaid 11.15 layout-renderer gradient defaults for Flowchart by seeding
    `themeVariables.useGradient`, `gradientStart`, and `gradientStop` for the local `base`, `dark`,
    `forest`, and `neutral` theme derivations.
  - Added the Mermaid 11.15 root-level Flowchart `<linearGradient id="<diagram-id>-gradient">`
    element when the effective theme enables gradients.
  - Targeted fresh `compare-svg-xml` filters passed for
    `upstream_cypress_conf_and_directives_spec_settings_from_directive_nodes_should_be_grey_004`,
    `upstream_docs_theming_customizing_themes_with_themevariables_003`,
    `upstream_docs_theming_diagram_specific_themes_002`,
    `stress_flowchart_theme_default_vs_base_base_075`,
    `upstream_cypress_flowchart_v2_spec_63_title_on_subgraphs_should_be_themeable_023`, and
    `upstream_docs_directives_declaring_directives_004`, all using
    `--upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`.
  - The remaining `conf_and_directives` theme filters all pass against the fresh 11.15 target:
    `upstream_cypress_conf_and_directives_spec_settings_from_directive_overriding_theme_variable_nodes_should_b_006`,
    `upstream_cypress_conf_and_directives_spec_settings_from_frontmatter_nodes_should_be_grey_005`,
    `upstream_cypress_conf_and_directives_spec_settings_from_initialize_and_directive_nodes_should_be_grey_007`,
    `upstream_cypress_conf_and_directives_spec_settings_from_initialize_overriding_themevariable_nodes_should_b_003`,
    `upstream_cypress_conf_and_directives_spec_should_render_if_values_are_not_quoted_properly_011`,
    `upstream_cypress_conf_and_directives_spec_theme_from_initialize_directive_overriding_theme_variable_nodes_008`,
    `upstream_cypress_conf_and_directives_spec_theme_from_initialize_frontmatter_overriding_theme_variable_dire_010`,
    `upstream_cypress_conf_and_directives_spec_theme_from_initialize_frontmatter_overriding_theme_variable_node_009`, and
    `upstream_cypress_conf_and_directives_spec_theme_variable_from_initialize_theme_from_directive_nodes_should_012`.
  - `cargo nextest run -p merman-core base_theme_derivation_matches_upstream_fixture_values forest_theme_derives_cscale_palette_like_upstream dark_theme_derives_peer_and_inverted_scales_like_upstream neutral_theme_derives_peer_and_label_scales_like_upstream`:
    passed, 4 tests.
  - `cargo nextest run -p merman-core dark_theme_derives_peer_and_inverted_scales_like_upstream`:
    passed, 1 test after aligning dark gradientStart serialization with upstream `#cccccc`.
  - `cargo nextest run -p merman-render flowchart_base_theme_renders_root_gradient`: passed,
    1 test.
  - `cargo nextest run -p merman-core theme`: passed, 4 tests.
  - `cargo nextest run -p merman-render flowchart`: passed, 78 tests.
  - `cargo fmt --check`: passed.
  - `git diff --check`: passed.
  - `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`:
    failed with 67 canonical XML mismatches plus the existing `flowchart-elk` local layout failure.
    This is a reduction from 95 fresh mismatches before the slice.
  - Remaining mismatch classification after this slice: `theme_config=0`, `newshapes=24`,
    `oldshapes=24`, `shape_alias=1`, `cluster=4`, `image_icon=4`, `edge=1`,
    `flow_node_data=3`, and `other=6`.

## Evidence Anchors

- `docs/workstreams/flowchart-11-15-svg-convergence/DESIGN.md`
- `docs/workstreams/flowchart-11-15-svg-convergence/TODO.md`
- `docs/workstreams/flowchart-11-15-svg-convergence/MILESTONES.md`
- `docs/workstreams/mermaid-11-15-complete-adaptation/EVIDENCE_AND_GATES.md`
- `target/upstream-svgs-11-15-flowchart`
- `target/compare/flowchart_report_parity.md`

## Notes

Do not treat stored Flowchart baseline failures as authoritative until the fresh target gate has
been used to classify the current slice. Do not bulk-refresh stored Flowchart baselines while the
fresh target still shows renderer DOM drift.
