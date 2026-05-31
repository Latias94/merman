# Flowchart 11.15 SVG Convergence - Evidence And Gates

Status: Active
Last updated: 2026-06-01

## Smallest Current Repro

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
```

This currently fails against the fresh Mermaid 11.15 Flowchart target with hundreds of DOM
mismatches plus one unsupported `flowchart-elk` local layout failure.

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
