# Swimlane Minimum (Mermaid@11.16.1)

This document records the admitted local support for Mermaid `swimlane-beta`.

Upstream references at pinned Mermaid 11.16.1:

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
- Typed rendering:
  - constructs one Flowchart semantic model and selects a dedicated `Swimlane` render artifact
    when the effective layout is `swimlane`
  - preserves explicit `dagre` and `elk` overrides by selecting the corresponding Flowchart
    artifact
- Layout:
  - ports the Mermaid 11.16 preparation, Sugiyama layering, lane bounds, orthogonal routing, and
    direction-aware post-processing pipeline
  - materializes edge-label nodes, synthetic default lanes, lane title bands, terminal stubs, and
    crossing-aware routes without a browser DOM
- SVG:
  - emits Mermaid-compatible swimlane cluster structure, vertical lane titles, markers, edge
    labels, and accessibility metadata
  - applies `arc`, `gap`, and disabled line-hop modes after edge construction while preserving
    canonical edge data points
- Fixtures:
  - normalized semantic and layout evidence exists under `fixtures/swimlane/`
  - all 30 Mermaid 11.16 DDLT swimlane inputs pass finite orthogonal layout and valid-SVG gates
  - three normalized fixtures have attested Mermaid 11.16 upstream SVG baselines

## Admission State

`swimlane` is admitted to the primary SVG matrix:

- semantic JSON fixtures are normalized under `fixtures/swimlane/`
- typed layout and SVG rendering run through the canonical render operation
- upstream baselines are provenance-locked to `@mermaid-js/mermaid-cli@11.16.0`,
  `mermaid@11.16.1`, and Headless Chrome 131
- `xtask compare-swimlane-svgs` is registered through the shared verification-fact catalog
- the default DOM comparison mode is `parity`; no family-specific comparator normalization is used

The admission gate is:

```sh
cargo run -p xtask -- compare-swimlane-svgs --check-dom --dom-mode parity
```

## Architecture

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

The local implementation therefore reuses Flowchart parsing but does not treat the Swimlane header
as a cosmetic alias. Artifact selection follows the effective layout, and the dedicated layout
pipeline owns lane geometry and routing before the shared Flowchart SVG emitter renders it.

## Identifier Ordering

Swimlane ordering must be stable across native and WebAssembly hosts. Merman therefore uses a
lexicographic UTF-16 code-unit comparison as its identifier tie-break, matching JavaScript's
ordinary string-order domain without embedding a locale database. The comparison is total,
deterministic, and independent of the process locale.

Pinned Mermaid paths that call `localeCompare` can choose a different order for mixed case,
diacritics, or non-Latin identifiers according to the browser's ICU build and locale. Merman does
not treat that host-sensitive collation as a semantic dependency of the layout backend. Fixtures
cover ASCII and representative Unicode tie-breaks so changes to the deterministic contract remain
visible.

## Residual Boundary

Browser font rasterization, `getBBox()` floats, locale-sensitive identifier collation, and pixel
snapshots remain bounded headless residuals under the repository-wide parity policy. They are not
normalized away by the Swimlane comparator. A coordinate-only mismatch caused by a different
locale tie-break must be attributed as such; semantic, DOM-structure, lane-membership, or routing
regressions still require a source-backed fix.
