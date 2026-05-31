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
bolt/lightning-bolt, and crossed-circle/summary. The theme-gradient slice added Mermaid 11.15
`useGradient` theme defaults plus the root Flowchart `<linearGradient>` element for `base`, `dark`,
`forest`, and `neutral` themes. The node-label slice added `noteLabel`, wrapped SVG markdown node
labels when `htmlLabels=false`, and preserved hourglass/collate parsed label type after clearing
displayed labels. The stacked-rectangle/procs slice aligned Mermaid 11.15 `multiRect.ts` classic
merged-path grouping. The HTML label slice restored Mermaid 11.15 single-image paragraph wrappers
and trimmed shapeData markdown block trailing newlines. The latest supported-DOM closeout slice
aligned non-markdown subgraph title wrapping, empty subgraph node ids, non-markdown edge label
paragraph wrappers, and literal `\n` handling in `nonMarkdownToHTML`. The supported fresh Flowchart
comparison now passes with zero canonical XML mismatches and one documented skip for the unsupported
`flowchart-elk` local layout family.

## Active Task

- Task ID: F115-080
- Owner: codex
- Files: `fixtures/upstream-svgs/flowchart`, `crates/xtask/src/cmd/compare`,
  `crates/merman-render/src/svg/parity/flowchart`
- Validation: regenerate stored Flowchart upstream SVG baselines, run
  `compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`, and confirm the stored
  gate honors the same documented `flowchart-elk` out-of-matrix policy.
- Status: READY
- Review: Compare against fresh Mermaid 11.15 output before changing stored baselines. Keep stored
  baseline churn separate from renderer code when practical.
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
- Mermaid 11.15 Flowchart non-markdown HTML labels are not markdown-parsed by `*`/`_` alone; only
  `labelType=markdown` goes through `markdownToHTML(...)`.
- Mermaid 11.15 `nonMarkdownToHTML(...)` wraps non-empty edge labels in `<p>...</p>` and treats both
  literal `\n` and actual newlines as `<br />`.
- Mermaid 11.15 non-markdown subgraph titles route through deprecated `createLabel(...)` with
  effectively infinite width; markdown subgraph titles still route through `createText(...)`.
- `flowchart-elk` is explicitly out of the current supported headless Flowchart matrix. The
  `compare-svg-xml` gate skips only
  `flowchart/upstream_html_demos_flowchart_elk_flowchart_elk_001` with a local-policy reason until a
  dedicated ELK layout lane lands.

## Blockers

- No current blocker for the supported Flowchart matrix. ELK layout support remains a follow-on
  layout lane, not part of this DOM convergence lane.

## Next Recommended Action

Run F115-080: refresh stored Flowchart upstream SVG baselines, then run the stored Flowchart DOM
gate. Preserve the fresh gate:

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
```

It currently passes with `Mismatches (0)` and one documented `flowchart-elk` skip.
