# Flowchart 11.15 SVG Convergence - Handoff

Status: Active
Last updated: 2026-06-12

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
`flowchart-elk` local layout family. Stored Flowchart upstream SVG baselines have been refreshed to
Mermaid 11.15, and the four former parser-only KaTeX demo fixtures are now active `*_katex`
semantic/layout/SVG baselines. Both stored Flowchart gates pass.

## Active Task

- Task ID: F115-090
- Owner: planner
- Files: `docs/workstreams/flowchart-11-15-svg-convergence`,
  `docs/workstreams/mermaid-11-15-complete-adaptation`
- Validation: review/verify the Flowchart lane, update umbrella evidence, and either close this
  child lane or leave only explicit follow-on work such as ELK layout support.
- Status: READY
- Review: Do not reopen Flowchart DOM work unless a fresh or stored Flowchart gate regresses.
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
- The four former `*_parser_only_katex` Flowchart demo fixtures are now active `*_katex` fixtures
  with upstream SVG baselines and Node/Puppeteer KaTeX measurement. The shared `xtask` upstream SVG
  policy now only skips the existing Flowchart ellipse parser-only fixture in this family.

## Blockers

- No current blocker for the supported Flowchart matrix. ELK layout support remains a follow-on
  layout lane, not part of this DOM convergence lane.

## Next Recommended Action

Run F115-090 closeout for the Flowchart child lane, then continue the umbrella Mermaid 11.15
campaign with the remaining ER/Class failures. Preserve these Flowchart gates:

```bash
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --dom-mode parity --dom-decimals 3
```

All currently pass with only the documented `flowchart-elk` skip.
