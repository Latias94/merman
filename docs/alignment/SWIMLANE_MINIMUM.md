# Swimlane Minimum (Mermaid@11.16.0)

This document records the admitted local support for Mermaid `swimlane-beta`.

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
- upstream baselines are provenance-locked to Mermaid CLI 11.16.0 and Headless Chrome 131
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

## Identifier Collation

Mermaid's Swimlane ordering passes use `String.prototype.localeCompare(id, undefined)`
with the browser's default `en-US` locale. The Rust implementation uses ICU4X's baked
`en-US` collator so ordering is independent of the host process locale. This is a semantic
dependency of the layout backend, not an optional presentation feature.

The expected Unicode ordering in the renderer regression test was generated with the pinned
Puppeteer 23.11.1 headless Chromium 131 artifact:

```sh
node - <<'NODE'
const puppeteer = require('./tools/mermaid-cli/node_modules/puppeteer');
(async () => {
  const browser = await puppeteer.launch({
    headless: true,
    executablePath: process.env.CHROME_BIN,
    args: ['--no-sandbox'],
  });
  const page = await browser.newPage();
  console.log(await page.evaluate(() => {
    const ids = ['Z', 'z', 'ä', 'a', 'A', 'å', 'Å', 'é', 'e', 'E', 'ß', 'ss', '中', '阿', '😀', '🧪'];
    return ids.sort((a, b) => a.localeCompare(b, 'en-US'));
  }));
  await browser.close();
})();
NODE
```

Set `CHROME_BIN` to the locked Chrome 131 executable used by the upstream SVG provenance run.

## Residual Boundary

Browser font rasterization, `getBBox()` floats, and pixel snapshots remain bounded headless
residuals under the repository-wide parity policy. They are not normalized away by the Swimlane
comparator. Any future DOM or geometry mismatch must be resolved from Mermaid source behavior or
documented as a browser-dependent residual.
