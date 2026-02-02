# Alignment Status (Mermaid Parity Dashboard)

Baseline: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

This file is a lightweight dashboard of what is currently implemented and what is covered by
goldens/baselines. It is intentionally short and should stay true even as fixtures grow.

## Golden Layers

- Semantic snapshots (`fixtures/**/*.golden.json`):
  - Scope: parsing + semantic model output.
  - Validator: `cargo nextest run -p merman-core` (snapshot test) or full `cargo nextest run`.
  - Maintenance: `cargo run -p xtask -- update-snapshots`.
- Layout snapshots (`fixtures/**/*.layout.golden.json`):
  - Scope: geometry layer (nodes/edges/clusters/labels/bounds).
  - Validator: `cargo nextest run -p merman-render` (layout snapshot test) or full `cargo nextest run`.
  - Maintenance: `cargo run -p xtask -- update-layout-snapshots [--diagram <name>]`.
- Upstream SVG baselines (`fixtures/upstream-svgs/**`):
  - Scope: authoritative Mermaid end-to-end SVG output (generated via official CLI).
  - How-to: `docs/rendering/UPSTREAM_SVG_BASELINES.md`.

## Diagram Coverage Matrix

Legend:

- Parse: `Engine::parse_diagram` supports the diagram and is covered by semantic snapshots.
- Layout: `layout_parsed` supports the diagram and is covered by layout snapshots.
- Render: a Rust SVG renderer exists (may be “debug” stage vs. “parity” stage).
- Upstream SVG: upstream baselines are stored under `fixtures/upstream-svgs/<diagram>/`.
- Compare: an automated compare report exists against upstream baselines.

| Diagram | Parse | Layout | Render | Upstream SVG | Compare |
|---|---:|---:|---|---:|---:|
| ER | yes | yes | Stage B + debug | yes | yes (`xtask compare-er-svgs`) |
| Flowchart | yes | yes | Stage B + debug | yes | yes (`xtask compare-flowchart-svgs`) |
| State | yes | yes | Stage B + debug | yes | yes (`xtask compare-state-svgs`) |
| Class | yes | yes | Stage B + debug | yes | yes (`xtask compare-class-svgs`) |
| Sequence | yes | yes | Stage B + debug | yes | yes (`xtask compare-sequence-svgs`) |
| Info | yes | yes | Stage B | yes | yes (`xtask compare-info-svgs`) |
| Pie | yes | yes | Stage B | yes | yes (`xtask compare-pie-svgs`) |
| Packet | yes | yes | Stage B | yes | yes (`xtask compare-packet-svgs`) |
| Timeline | yes | yes | Stage B | yes | yes (`xtask compare-timeline-svgs`) |
| Journey | yes | yes | Stage B | yes | yes (`xtask compare-journey-svgs`) |
| Kanban | yes | yes | Stage B | yes | yes (`xtask compare-kanban-svgs`) |
| GitGraph | yes | yes | Stage B | yes | yes (`xtask compare-gitgraph-svgs`) |
| Gantt | yes | yes | Stage B | yes | yes (`xtask compare-gantt-svgs`) |
| C4 | yes | yes | Stage B | yes | yes (`xtask compare-c4-svgs`) |
| Block | yes | yes | Stage B | yes | yes (`xtask compare-block-svgs`) |
| Radar | yes | yes | Stage B | yes | yes (`xtask compare-radar-svgs`) |
| Treemap | yes | yes | Stage B | yes | yes (`xtask compare-treemap-svgs`) |
| XYChart | yes | yes | Stage B | yes | yes (`xtask compare-xychart-svgs`) |
| Mindmap | yes | yes | Stage B | yes | yes (`xtask compare-mindmap-svgs`) |
| Architecture | yes | yes | Stage B | yes | yes (`xtask compare-architecture-svgs`) |
| Requirement | yes | yes | Stage B | yes | yes (`xtask compare-requirement-svgs`) |
| QuadrantChart | yes | yes | Stage B | yes | yes (`xtask compare-quadrantchart-svgs`) |
| Sankey | yes | yes | Stage B | yes | yes (`xtask compare-sankey-svgs`, DOM parity-root mode) |

Recent progress: sequence `alt`/`loop` frames derive separator placement from layout message y-coordinates;
the dashed separators now use the exact same x-coordinates as the frame edges to match upstream SVG and
avoid sub-pixel gaps at the frame border.
As of 2026-02-01, `xtask compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` reports 0 DOM mismatches
for the current fixture set (diagram subtree parity).
As of 2026-02-02, `xtask compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` reports 79 DOM
mismatches out of 475 upstream SVG baselines (83.4% passing). Current parity-root mismatches are concentrated in
5 diagrams:

- Architecture: 20
- Class: 14
- GitGraph: 14
- Mindmap: 8
- State: 23

Most parity-root deltas are root `<svg>` viewport attributes (`style` max-width / `viewBox`) and are therefore
sensitive to upstream sizing policy, layout extents (including edge labels/groups), and floating-point rounding.
Recent progress (2026-02-02): mindmap headless layout now follows Mermaid node sizing rules (shape/padding/wrapping)
instead of a placeholder grid layout, and `xtask debug-mindmap-svg-positions` was added to compare upstream/local node
coordinates; the remaining parity-root mindmap mismatches are currently dominated by root `<svg>` viewport sizing.
Recent progress (2026-02-01): state diagram Stage B now derives root `viewBox`/`max-width` by parsing the emitted
SVG and approximating `svg.getBBox()` (ignoring placeholder boxes like `0x0` and `0.1x0.1` rects), fixing large
viewport blow-ups (e.g. floating notes fixtures) and reducing parity-root state mismatches.
Recent progress (2026-02-02): state diagram nested roots shift their local origin by Dagre's fixed 8px graph margin,
matching Mermaid’s recursive `dagre-wrapper` structure where nested cluster frames start at x/y=8 in the nested
coordinate space.
Recent progress (2026-02-01): state diagram dagre layout now uses Mermaid margins (`marginx/marginy=8`) for both the
top-level graph and extracted cluster graphs.
Recent progress (2026-02-01): state diagram dagre cluster extraction now matches Mermaid's `dagre-wrapper` more
closely by extracting any disconnected cluster (not only root-level), and by injecting the parent cluster node into
the extracted graph during the recursive layout pass so Dagre's compound border sizing yields Mermaid-like padding.
Recent progress (2026-02-02): state diagram layout now excludes legacy floating-note syntaxes that Mermaid parses but
does not render, so they no longer affect node/edge placement or root viewport sizing.
Recent progress (2026-02-02): state diagram label measurement now honors compiled CSS font overrides
(weight/size/family/italic), improving classDef-styled label width parity.
Recent progress (2026-02-02): C4 diagram Stage B now matches upstream root `viewBox` (DOM parity-root mode) by
mirroring Mermaid's `calculateTextWidth/Height` sizing and the `techn` measurement quirk in `c4Renderer.js`.
Recent progress (2026-02-02): pie Stage B now matches upstream root `viewBox` and root `style max-width` in
DOM parity-root mode by mirroring Mermaid's legend width sizing (BCR-like) rather than `getComputedTextLength()`,
including a small bbox overhang correction for a few glyph edge cases.
Recent progress (2026-02-01): state diagram cluster rendering no longer double-applies the 8px dagre margin during
SVG emission, aligning cluster frame placement with Mermaid and reducing parity-root mismatches.
Recent progress: architecture Stage B now computes root `viewBox`/`max-width` from emitted element bounds and honors
`architecture.padding`/`iconSize`/`fontSize`, fixing previously clipped non-empty Architecture SVG outputs. Root parity
still depends on matching upstream Cytoscape/FCoSE layout behavior.
Recent progress (2026-02-01): block diagram layout now models Mermaid’s mixed label metrics (HTML width + SVG bbox
height) and the block Stage B renderer now emits root `viewBox`/`max-width` using Mermaid-like diagram padding.
Recent progress: `xychart` headless layout and Stage B parity renderer exist and are validated against
upstream SVG baselines via `xtask compare-xychart-svgs` (DOM parity mode).
Recent progress: flowchart Stage B now matches upstream SVG DOM for the current fixture set in parity mode
(`cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`).
Recent progress: flowchart-v2 stadium/cylinder geometry now matches upstream more closely in strict SVG XML parity
(`xtask compare-svg-xml --diagram flowchart --dom-mode strict`) by modeling Chromium bbox quirks for cylinders and
using stadium render-dimensions for edge intersections.
Recent progress: flowchart-v2 hexagon edge intersections now mirror Mermaid `hexagon.ts` by generating polygon
points from render dimensions (text bbox + padding) while still using `updateNodeBounds(...)`-derived node sizes
for the `intersect.polygon(...)` transform, eliminating hexagon-related `data-points` drift.
Recent progress: flowchart-v2 `classDef` CSS rules now preserve Mermaid insertion order (IndexMap), matching upstream
`<style>` rule ordering in strict SVG XML parity (e.g. `bigger_font_from_classes_spec`).
Recent progress: flowchart-v2 strict SVG XML parity now stringifies `data-points` using ECMAScript-compatible float
formatting (via `ryu-js`) to match V8 tie-breaking in shortest round-trippable decimals.
Recent progress: flowchart-v2 `linkStyle ... interpolate ...` now trims the whitespace between the curve name and the
first style token so rendered edge `style="...;;;..."` matches upstream strict SVG XML output.
Recent progress: flowchart-v2 cluster label positioning now derives the SVG title bbox width from the rendered
`<text>/<tspan>` lines (not the layout placeholder metrics), improving strict SVG XML parity for wrapped titles.
Recent progress: flowchart-v2 cluster edge labels are positioned using the cut edge polyline midpoint (mirroring
Mermaid’s `cutPathAtIntersect` + `calcLabelPosition`), improving strict SVG XML parity for subgraph outgoing links.
Recent progress: flowchart Dagre config now uses Mermaid margins (`marginx/marginy=8`) for both the top-level
graph and extracted cluster graphs, improving subgraph/cluster geometry parity.
Recent progress: `dugong` now matches dagrejs graph defaults (`edgesep=20`), improving multiedge routing parity
and reducing Flowchart root viewport drift.
Recent progress: flowchart-v2 headless text measurement strips `fa:fa-*` / `fas:fa-*` tokens for HTML labels so
icon placeholders don’t inflate node/cluster bbox in exported SVG baselines where FontAwesome CSS is absent.
Recent progress: flowchart-v2 strict SVG XML parity now matches Mermaid’s DOM insertion order more closely by
partitioning cluster endpoint edges to the end (mirroring `adjustClustersAndEdges` remove+readd behavior) and by
ordering cluster boxes consistently (ancestor-first, then reverse subgraph registration order).
Recent progress: flowchart-v2 renders empty subgraphs (node-like subgraph declarations) before extracted cluster
root groups inside `.nodes`, matching upstream DOM order in `outgoing_links_4_spec`.
Recent progress: flowchart-v2 now renders self-loop label placeholder nodes for cluster nodes (e.g. `C1---C1---{1,2}`)
before the nested extracted cluster `.root` group, matching upstream DOM order in `upstream_flowchart_v2_self_loops_spec`.
Recent progress: flowchart HTML `<img>` labels now contribute to foreignObject sizing (instead of being treated as
empty text), matching upstream SVG baselines for image-only and mixed image+text nodes.
Recent progress: flowchart Markdown measurement in HTML-like mode now accounts for `<strong>/<em>` styling deltas
(including nested tags) and updates a few vendored HTML override widths, fixing `upstream_markdown_strings` and
`upstream_markdown_subgraphs` in strict SVG XML parity.
Recent progress: flowchart-v2 extracted cluster root groups now follow Mermaid’s sibling ordering more closely by
sorting in reverse subgraph definition order (mirrors Dagre child registration behavior in upstream SVG DOM).
Recent progress: flowchart-v2 subgraph `style` statements now apply to the cluster `<rect>` and title `<span>`
styles (including `color: ... !important` on the label), matching Mermaid’s styled subgraph semantics.
Recent progress: flowchart-v2 `theme: base` now derives missing `themeVariables` (colors and dark mode) and feeds
them into the generated CSS (cluster/title/edgeLabel), fixing `subgraph_title_themeable_spec` in strict SVG XML parity.
Recent progress: flowchart-v2 `data-points` strict parity no longer relies on a global fixed-point quantization;
instead we normalize Dagre self-loop control points (snapping dummy placement to the common 1/64px grid when it is
already extremely close) and apply a very narrow truncation heuristic only for coordinates extremely close to `1/3`
or `2/3` remainders at the 2^18 scale, preserving prior Markdown strict parity fixes.
Recent progress: flowchart-v2 cluster title HTML label widths now include a few additional Trebuchet metrics overrides
("Foo SubGraph", "Bar SubGraph", "Main") so `foreignObject width` and `cluster-label translate(...)` match upstream.
Recent progress: flowchart-v2 `data-points` now snaps coordinates that are extremely close to their f32-rounded value,
while preserving the common `next_up(f32)` rounding artifacts seen in upstream baselines (e.g. `...0001`).
As of 2026-01-27, `xtask compare-svg-xml --diagram flowchart --dom-mode strict --dom-decimals 3` reports 0 flowchart
mismatches.
As of 2026-01-29, `xtask compare-svg-xml --diagram requirement --dom-mode strict --dom-decimals 3` reports 0
requirement mismatches (for the pinned Mermaid@11.12.2 upstream baselines).
As of 2026-01-29, `xtask compare-svg-xml --diagram gantt --dom-mode strict --dom-decimals 3` reports 0 gantt mismatches.
As of 2026-02-02, `xtask compare-svg-xml --dom-mode strict --dom-decimals 3` reports 175 total strict XML mismatches
(state=43, architecture=25, block=22, class=16, kanban=15, gitgraph=14, mindmap=11, pie=11, xychart=11, c4=7).
Strict XML 0-mismatch diagrams: er, flowchart, gantt, info, journey, packet, quadrantchart, radar, requirement, sankey,
timeline, treemap.
See `docs/alignment/FLOWCHART_SVG_STRICT_XML_GAPS.md` for a workflow to debug float-level `data-points` drift when
new fixtures are introduced.
Recent progress: flowchart fixtures now cover `flow-style.spec.js` and `flow-interactions.spec.js` more
thoroughly (style/class edge cases, click syntax matrix, and `securityLevel: loose` callback gating).
Recent progress: flowchart edge curves now cover `monotoneX`/`monotoneY` and `step`/`stepBefore` in addition to
`linear`/`stepAfter`/`basis`/`cardinal`, and the fixture set covers `linkStyle ... stroke-width:1px;` variants.
Recent progress: sequence headless layout now models notes and `rect` blocks as layout nodes (`note-*`, `rect-*`),
so SVG viewBox/bounds can expand to match upstream baselines (e.g. left-of notes and nested rect blocks).
Recent progress: sequence headless layout now models self-messages with `startx == stopx` and adds the extra
vertical bump Mermaid applies for the loop curve; Stage B SVG renders self-messages as `<path>` and renders
participant types (`boundary`, `control`, `entity`, `database`, `collections`, `queue`) with Mermaid-like DOM
structure (the `participant_types` upstream baseline now matches in DOM parity mode).
Recent progress: sequence Stage B now renders `opt`/`par` blocks (including `par over`) and `box` frames;
empty block labels are rendered as a zero-width space (matching upstream SVG behavior).
Recent progress: sequence Stage B now treats HTML `<br>` variants as line breaks in participant labels, notes,
and message texts, matching upstream DOM structure in `html_br_variants_and_wrap`; empty message labels
(trailing colon) now still produce a message text node like upstream.
Recent progress: sequence Stage B now matches upstream SVG DOM for the current fixture set in parity mode
(`cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`).
Recent progress: `info` and `pie` Stage B parity renderers exist and are validated against upstream
SVG baselines via `xtask compare-info-svgs` / `xtask compare-pie-svgs`.
Recent progress: `packet` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-packet-svgs`.
Recent progress: `timeline` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-timeline-svgs`.
Recent progress: `journey` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-journey-svgs`.
Recent progress: `kanban` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-kanban-svgs`.
Recent progress: `gitGraph` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-gitgraph-svgs`.
Recent progress: `gantt` Stage B parity renderer exists and is validated against upstream SVG
baselines via `xtask compare-gantt-svgs`.
Recent progress: `block` headless layout and Stage B SVG renderer exist and are validated against
upstream SVG baselines via `xtask compare-block-svgs` (DOM parity mode).
Recent progress: `radar` headless layout and Stage B SVG renderer exist and are validated against
upstream SVG baselines via `xtask compare-radar-svgs` (DOM parity mode).
Recent progress: `sankey` headless layout and Stage B SVG renderer exist and are validated against
upstream SVG baselines via `xtask compare-sankey-svgs` (DOM parity-root mode).
Recent progress: `xtask compare-all-svgs --check-dom` now runs end-to-end (class/state/gantt
compare tasks now honor `--check-dom`), and the state layout goldens were refreshed after
aligning default text style (16px) and node padding behavior.
Recent progress: Architecture Cypress fixtures that use legacy shorthand syntax now have
CLI-compatible `*_normalized` variants so we can store upstream CLI SVG baselines and run DOM parity
checks without losing the original Cypress strings.

## Alignment Sanity Checks

- Internal consistency: `cargo run -p xtask -- check-alignment`
  - ensures every fixture has a `.golden.json`
  - ensures coverage docs reference existing local paths
- Full SVG parity sweep (aggregated): `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
- Debug viewport bounds for a single SVG: `cargo run -p xtask -- debug-svg-bbox --svg <path> --padding 8`
