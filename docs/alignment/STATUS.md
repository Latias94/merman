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
Recent progress: `xychart` headless layout and Stage B parity renderer exist and are validated against
upstream SVG baselines via `xtask compare-xychart-svgs` (DOM parity mode).
Recent progress: flowchart Stage B now matches upstream SVG DOM for the current fixture set in parity mode
(`cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`).
Recent progress: flowchart-v2 stadium/cylinder geometry now matches upstream more closely in strict SVG XML parity
(`xtask compare-svg-xml --diagram flowchart --dom-mode strict`) by modeling Chromium bbox quirks for cylinders and
using stadium render-dimensions for edge intersections.
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
Recent progress: flowchart HTML `<img>` labels now contribute to foreignObject sizing (instead of being treated as
empty text), matching upstream SVG baselines for image-only and mixed image+text nodes.
Recent progress: flowchart Markdown measurement in HTML-like mode now ignores `<strong>/<em>` width deltas, matching
upstream foreignObject bbox sizing more closely (fixes `upstream_markdown_strings` in strict SVG XML parity).
As of 2026-01-26, `xtask compare-svg-xml --diagram flowchart --dom-mode strict --dom-decimals 3` reports 19 flowchart
mismatches remaining; see `target/compare/xml/xml_report.md` for the current list.
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
