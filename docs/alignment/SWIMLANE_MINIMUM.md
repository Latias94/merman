# Swimlane Minimum (Mermaid@11.16.0)

This document tracks the staged local support slice for Mermaid `swimlane-beta`.

Upstream references at pinned Mermaid 11.16.0:

- Detector: `packages/mermaid/src/diagrams/swimlanes/detector.ts`
- Diagram adapter: `packages/mermaid/src/diagrams/swimlanes/swimlanesDiagram.ts`
- Styles: `packages/mermaid/src/diagrams/swimlanes/styles.ts`
- Flowchart adapter behavior: `packages/mermaid/src/diagrams/flowchart/flowDiagram.ts`
- Layout backend: `packages/mermaid/src/rendering-util/layout-algorithms/swimlanes/`
- Swimlane cluster renderer: `packages/mermaid/src/rendering-util/rendering-elements/clusters/swimlane.js`

## Implemented

- Detection:
  - accepts `swimlane-beta`
  - exposes internal diagram id `swimlane`
- Parser/model:
  - reuses the Flowchart parser and DB semantics, matching upstream `createFlowDiagram`
  - preserves the `swimlane-beta` keyword in the semantic JSON
  - emits `type: "swimlane"` while retaining Flowchart nodes, edges, subgraphs, classes, styles,
    accessibility fields, and warning facts
- Config:
  - sets effective `layout` to `swimlane` for swimlane diagrams when the user did not explicitly
    override layout
  - preserves user layout overrides
  - includes Mermaid 11.16 swimlane config defaults: `lineHops`, `ignoreCrossLaneEdges`,
    `optimizeRanksByCrossings`, `automaticLaneOrdering`, and `useMaxWidth`
- LSP/editor facts:
  - reuses Flowchart editor facts, preserving node/source-span semantics for completions and
    navigation
- Fixtures:
  - semantic fixture coverage exists under `fixtures/swimlane/`

## Admission State

`swimlane` is recorded as `ParseOnly` in the admission inventory:

- semantic JSON fixtures are normalized under `fixtures/swimlane/`
- layout goldens are not admitted
- local SVG rendering is intentionally not admitted
- upstream SVG baselines and a family-local compare command are deferred until a source-backed
  swimlane layout port exists

## Why Rendering Is Staged

Upstream Swimlane is not plain Flowchart rendering with a different header. It is a layout-variant
diagram that reuses Flowchart parsing and rendering, but swaps in a dedicated swimlane layout
backend. That backend includes:

- `prepareLayoutForSwimlanes`
- edge-label node transformation
- lane-aware layering and rank optimization
- optional automatic lane ordering
- orthogonal edge routing
- line-hop post-processing
- swimlane cluster shape metadata and lane content alignment

Rendering `swimlane-beta` through the ordinary local Flowchart/Dagre path would produce an SVG, but
it would not represent Mermaid 11.16 swimlane semantics. The current staged state is deliberate.

## Known Gaps

- No typed render parser for `swimlane` yet.
- No local port of `rendering-util/layout-algorithms/swimlanes/` yet.
- No swimlane-specific layout goldens or upstream SVG baselines yet.
- No dedicated `xtask compare-swimlane-svgs` command yet.
- Mermaid issue https://github.com/mermaid-js/mermaid/issues/7954 tracks a separate upstream
  11.16.0 Flowchart subgraph-arrow regression. Do not use that regression as a reason to broaden
  local Flowchart/Swimlane comparator normalization.
