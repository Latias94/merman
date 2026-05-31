# Flowchart 11.15 SVG Convergence - Handoff

Status: Active
Last updated: 2026-06-01

## Current State

This lane was split from M15C-060 after fresh Mermaid 11.15 evidence showed Flowchart is a large DOM
convergence effort, not a one-fixture MathML baseline refresh. F115-020 and F115-030 landed the
first convergence slice: Flowchart 11.15 defs/markers/scoped ids/`data-look`, first-order
`outer-path` class surfaces, SVG-label row semantics, centered edge-label anchors, cluster id
scoping, and root-first edge/cluster `htmlLabels` fallback behavior. The latest F115-040/F115-050
slice aligned `shapeData` markdown-label defaults, normal node root `htmlLabels` semantics,
markdown node label classes, icon/image label spans, and classic hexagon's 11.15 6-point polygon
model. The adjacent no-label shape slice added upstream `outer-path` classes for stop/framed-circle,
bolt/lightning-bolt, and crossed-circle/summary. The latest theme-gradient slice added Mermaid
11.15 `useGradient` theme defaults plus the root Flowchart `<linearGradient>` element for `base`,
`dark`, `forest`, and `neutral` themes. The latest node-label slice added `noteLabel`, wrapped SVG
markdown node labels when `htmlLabels=false`, and preserved hourglass/collate parsed label type
after clearing displayed labels. The latest stacked-rectangle/procs slice aligned Mermaid 11.15
`multiRect.ts` classic merged-path grouping. The targeted fresh probes pass. Full fresh Flowchart
comparison is still red with 15 mismatches and one unsupported `flowchart-elk` local layout failure.

## Active Task

- Task ID: F115-040/F115-050 overlap
- Owner: codex
- Files: `crates/merman-core/src/diagrams/flowchart`, `crates/merman-render/src/flowchart`,
  `crates/merman-render/src/svg/parity/flowchart`
- Validation: targeted fresh `compare-svg-xml --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart`
  filters for shape matrix fixtures, config/directive styling, image/icon labels, cluster labels,
  and remaining special-shape cases; then `cargo nextest run -p merman-render flowchart`
- Status: IN_PROGRESS
- Review: Compare against fresh Mermaid 11.15 output before changing stored baselines. Keep stored
  Flowchart baseline refresh blocked until the fresh gate is green or skips are documented.
- Evidence: `docs/workstreams/flowchart-11-15-svg-convergence/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Flowchart must be a child lane because fresh 11.15 output exposes 594 DOM mismatches.
- The old stored Math fixture was stale for MathML `columnalign`, but fixing that does not make
  Flowchart green.
- Fresh-target comparison is the authoritative gate for renderer work; stored Flowchart baselines
  are downstream evidence only.
- `flowchart-elk` is not covered by the current local layout path and needs an explicit support,
  skip, or split decision.
- Mermaid 11.15 preserves bare backtick-wrapped pipe edge labels as plain SVG text instead of
  dropping them as an empty code span.
- Mermaid 11.15 is asymmetric for Flowchart `htmlLabels`: normal node shapes read root
  `htmlLabels` directly in `labelHelper`, while edge and cluster labels use
  `getEffectiveHtmlLabels(...)` and still honor deprecated `flowchart.htmlLabels`.
- Mermaid 11.15 `shapeData` labels default to markdown unless an explicit `labelType` is provided.
- Classic hexagon is a 6-point polygon in Mermaid 11.15; RoughJS path output is only for
  `look=handDrawn`.
- No-label special shapes are not uniform: `stop`, `bolt`, and `crossed-circle` use an `outer-path`
  wrapper, while `filled-circle` remains a bare group.

## Blockers

- None for F115-050.
- Full lane closeout is blocked until `flowchart-elk` policy is decided.

## Next Recommended Action

Continue F115-050/F115-060 by reducing the remaining 15 fresh mismatches. The next high-value
targets are image/icon HTML labels, cluster/subgraph label structure, edge label placement, flow
node data, and SVG-like escaped tag handling. Keep `flowchart-elk` as a required F115-070 policy
decision before stored Flowchart baseline refresh.
