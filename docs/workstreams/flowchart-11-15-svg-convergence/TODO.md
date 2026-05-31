# Flowchart 11.15 SVG Convergence - TODO

Status: Active
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

- [x] F115-010 [owner=planner] [deps=none] [scope=docs/workstreams/flowchart-11-15-svg-convergence]
  Goal: Freeze the fresh Mermaid 11.15 Flowchart gap model and split this child lane from the
  umbrella M15C-060 task.
  Validation: DESIGN.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, HANDOFF.md, and
  CONTEXT.jsonl exist and agree.
  Evidence: `docs/workstreams/flowchart-11-15-svg-convergence/EVIDENCE_AND_GATES.md`
  Context: `docs/workstreams/flowchart-11-15-svg-convergence/CONTEXT.jsonl`
  Handoff: DONE. Fresh Flowchart 11.15 comparison shows 594 DOM mismatches plus one unsupported
  `flowchart-elk` local layout failure.

## M1 - DOM Envelope And Identity

- [x] F115-020 [owner=codex] [deps=F115-010] [scope=crates/merman-render/src/svg/parity/flowchart]
  Goal: Match Mermaid 11.15 Flowchart's renderer envelope for defs, margin markers, drop shadows,
  `data-look`, scoped node and edge ids, classic rounded-rect output, and first-order shape path
  class surfaces.
  Validation: targeted `compare-svg-xml --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart`
  filters covering `upstream_docs_math_flowcharts_001`, `stress_flowchart_classdef_and_inline_classes_003`,
  and `stress_flowchart_clicks_and_tooltips_005`; `cargo nextest run -p merman-render flowchart`.
  Review: Confirm the slice reduces fresh 11.15 mismatches without depending on stale stored
  Flowchart baselines.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus Mermaid 11.15 flowchart renderer source in `repo-ref/mermaid`.
  Handoff: DONE. Targeted Math, special-shape class/style, clickable/tooltip, cluster-id, and
  htmlLabels precedence probes pass against the fresh 11.15 target. Full fresh Flowchart comparison
  is down from 594 to 359 mismatches plus one `flowchart-elk` layout failure.

- [x] F115-030 [owner=codex] [deps=F115-020] [scope=crates/merman-render/src/svg/parity/flowchart]
  Goal: Match Mermaid 11.15 markdown/text row DOM for edge and node labels when `htmlLabels=false`,
  including `row text-outer-tspan` wrappers and nested `text-inner-tspan` spans.
  Validation: targeted fresh compare filters that currently classify as `edge_markdown_rows` and
  `missing_row_class`.
  Review: Keep text measurement behavior unchanged unless fixture evidence proves a layout delta.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/workstreams/flowchart-text-style-parity`.
  Handoff: DONE. `probe_flowchart_edge_markdown_html_false_982` and
  `probe_flowchart_edge_quoted_markdown_html_false_985` pass against the fresh 11.15 target. Local
  SVG-label tspans now carry Mermaid 11.15 `row` class semantics and centered edge-label anchors.

## M2 - Shape, Label, And Cluster Convergence

- [ ] F115-040 [owner=codex] [deps=F115-020] [scope=crates/merman-render/src/svg/parity/flowchart]
  Goal: Close remaining 11.15 shape path and label-container class mismatches, including
  `outer-path` coverage for non-rect special shapes.
  Validation: targeted fresh compare filters covering representative special shapes; mismatch
  classifier shows `outer_path_class` and `shape_path_class` materially reduced.
  Review: Prefer matching upstream shape helpers over fixture-specific patches.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream.
  Handoff: IN_PROGRESS. First-order `outer-path` class coverage landed for representative special
  shapes; residual shape path/class mismatches remain in the fresh full Flowchart gate.

- [ ] F115-050 [owner=codex] [deps=F115-030,F115-040] [scope=crates/merman-render/src/svg/parity/flowchart]
  Goal: Match Mermaid 11.15 HTML/`foreignObject` label DOM surfaces that dominate the fresh
  `html_foreign_object` category.
  Validation: targeted fresh compare filters for node labels, edge labels, markdown labels, and
  `htmlLabels` true/false variants.
  Review: Do not introduce a browser layout dependency; preserve deterministic local measurement.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/workstreams/flowchart-text-style-parity`.
  Handoff: IN_PROGRESS. The first config-semantics slice is landed: Flowchart labels now follow
  Mermaid 11.15 root-first `htmlLabels` precedence, with `flowchart.htmlLabels` as deprecated
  fallback. Remaining work is the dominant HTML/`foreignObject` label DOM surface.

- [ ] F115-060 [owner=codex] [deps=F115-020] [scope=crates/merman-render/src/svg/parity/flowchart]
  Goal: Match Mermaid 11.15 subgraph cluster group structure, namespace ids, labels, and class
  ordering for supported Flowchart layouts.
  Validation: targeted fresh compare filters for subgraph-heavy fixtures; full fresh Flowchart
  mismatch count no longer reports every case as `subgraph_cluster`.
  Review: Keep layout geometry changes separate from DOM ordering and class-string changes.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream.
  Handoff: Not started.

- [ ] F115-070 [owner=codex] [deps=F115-020,F115-050] [scope=crates/merman-render/src/svg/parity/flowchart,crates/xtask/src/cmd/compare]
  Goal: Close clickable/tooltip wrapper deltas and decide the `flowchart-elk` fixture policy.
  Validation: targeted fresh compare filters for clickable Flowchart fixtures; one of these is true:
  `flowchart-elk` is supported, explicitly skipped in upstream SVG gates with rationale, or split to
  a separate ELK layout workstream.
  Review: Any skip must be narrow and documented in the umbrella evidence.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus upstream fixture metadata.
  Handoff: Not started.

## M3 - Fresh Full Gate And Stored Baseline Refresh

- [ ] F115-080 [owner=codex] [deps=F115-030,F115-040,F115-050,F115-060,F115-070] [scope=fixtures/upstream-svgs/flowchart,crates/merman-render/src/svg/parity/flowchart]
  Goal: Make the supported Flowchart corpus green against fresh Mermaid 11.15 output, then refresh
  stored Flowchart upstream SVG baselines.
  Validation: `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`;
  `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out fixtures/upstream-svgs`;
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`.
  Review: Stored baseline churn must be staged separately from renderer code when practical.
  Evidence: `EVIDENCE_AND_GATES.md`
  Context: this workstream plus `docs/rendering/UPSTREAM_SVG_BASELINES.md`.
  Handoff: Not started.

## M4 - Closeout And Umbrella Reintegration

- [ ] F115-090 [owner=planner] [deps=F115-080] [scope=docs/workstreams/flowchart-11-15-svg-convergence,docs/workstreams/mermaid-11-15-complete-adaptation]
  Goal: Close this child lane or split any remaining Flowchart 11.15 work into narrower lanes, then
  update M15C-060 evidence.
  Validation: `review-workstream`; `verify-rust-workstream`; umbrella full parity gate re-run or
  documented remaining non-Flowchart failures.
  Review: No partial renderer convergence may be reported as complete without fresh gate evidence.
  Evidence: this workstream and `docs/workstreams/mermaid-11-15-complete-adaptation/EVIDENCE_AND_GATES.md`
  Context: this workstream and umbrella workstream.
  Handoff: Not started.
