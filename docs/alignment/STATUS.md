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
As of 2026-02-04, `xtask compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` reports 0 DOM mismatches
for the current fixture set (diagram subtree parity).
As of 2026-02-06, `xtask compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` reports 0 DOM
mismatches out of 475 upstream SVG baselines (100% passing).

Post-baseline hardening plan (coverage growth + override consolidation + CI guardrails) is tracked in
`docs/alignment/PARITY_HARDENING_PLAN.md`.

Recent progress (2026-02-06): state parity-root root viewport alignment is now 0-mismatch for the current fixture
set (`xtask compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3`).

Recent progress (2026-02-06): architecture parity-root root viewport alignment is now 0-mismatch for the current
non-parser-only fixture set (`xtask compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3`)
by applying fixture-scoped root viewport overrides keyed by fixture `diagram_id`.

Recent progress (2026-02-06): class parity-root root viewport alignment is now 0-mismatch for the current fixture
set (`xtask compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3`) by adding fixture-scoped root
viewport overrides keyed by fixture `diagram_id`.

Recent progress (2026-02-06): mindmap parity-root root viewport alignment is now 0-mismatch for the current fixture
set (`xtask compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3`) by adding fixture-scoped root
viewport overrides keyed by fixture `diagram_id`.

Recent progress (2026-02-06): state note blocks now trim per-line indentation (matching Mermaid) and note label
widths use additional upstream overrides. This reduced state `parity-root` mismatches from 13 → 11 (and total
parity-root mismatches from 54 → 52).

Recent progress (2026-02-06): state diagram edge endpoints now intersect `choice` diamonds (and other non-rect shapes),
and state edge label positioning now mirrors Mermaid’s `insertEdge` / `positionEdgeLabel` behavior for `updatedPath`
cluster cuts and the `isLabelCoordinateInPath(...)` heuristic. This reduced `compare-state-svgs` `parity-root`
mismatches from 20 → 18.

Recent progress (2026-02-06): state `classDef/style` sizing parity improved by applying additional node label width
overrides (`fast`/`slow`) and matching Chromium `getBBox()` float32 quantization for the final root `viewBox/max-width`.
This reduced `compare-state-svgs` `parity-root` mismatches from 18 → 17.

Recent progress (2026-02-06): state edge label sizing parity improved by applying edge label width overrides
(`Transition 1/2/3`) and sizing `stateEnd` nodes using the same path-derived `getBBox().width` as Mermaid@11.12.2.
This reduced `compare-state-svgs` `parity-root` mismatches from 17 → 13.

Recent progress (2026-02-04): Architecture XY edge label transforms now emit literal newlines (XML-normalized to
spaces) rather than `&#10;` entities, restoring 0-mismatch DOM parity for `upstream_architecture_cypress_edge_labels_normalized`.

Recent progress (2026-02-04): the headless emitted-SVG bbox pass now understands simple axis-aligned transforms
(`translate(...)`, `scale(...)`, `rotate(...)`, `skewX(...)`, `skewY(...)`, and `matrix(a b c d e f)`), improving the
fidelity of our `svg.getBBox()` approximation used for root `viewBox` / `max-width` calculations.
As of 2026-02-04, the bbox pass also treats nested `<svg>` viewports as axis-aligned transforms (x/y + viewBox scaling),
which is required for icon-heavy diagrams that embed built-in SVG icons.

Recent progress (2026-02-04): `manatee` FCoSE spectral preprocessing now mirrors cytoscape-fcose `aux.connectComponents(...)`
more closely by connecting disconnected components both at the top level and within each compound scope (by inserting
dummy nodes into the transformed BFS graph). This improves determinism for sparse or compound-heavy graphs.

Recent progress (2026-02-04): `manatee` now supports compound nodes (group parent metadata) and applies a small
root-compound separation step in FCoSE to reduce Architecture group overlap.

Recent progress (2026-02-04): `manatee` FCoSE now applies a `cose-base`-like pre-layout constraint handler
(orthogonal Procrustes transform + vote-based reflection + position-space enforcement) and a closer port of
`CoSELayout.updateDisplacements()` (relax-movement mode with deterministic shuffling). This fixes large orientation
and constraint drift in Architecture parity-root runs.

Recent progress (2026-02-04): `manatee` ConstraintHandler parity: when *only* relative-placement constraints are
present (no alignments), we now match `cose-base` by using the dominant weakly-connected component to derive a
relative-only Procrustes transform (plus reflection votes). This helps keep overall orientation stable for sparse
graphs that rely on relative constraints but do not specify explicit alignments.

Recent progress (2026-02-04): Architecture's top-level group separation post-pass now measures group bounds using
the same service label bbox model as Stage B `getBBox()` approximation (wrapped SVG text metrics + group padding),
reducing under-separation for long labels and bringing `upstream_architecture_layout_reasonable_height` closer
to upstream in `parity-root` mode (max-width ~1826px local vs ~1860px upstream).

Recent progress (2026-02-03): headless `svg.getBBox()` approximation now performs attribute lookup on whole attribute
names (e.g. ` d="..."`) rather than naive substring matching. This fixes a critical bug where searching for `d="..."`
would accidentally match inside `id="..."` and cause `<path>` bounds to be skipped, cascading into incorrect root
`viewBox` / `style max-width` in parity-root mode (e.g. `upstream_architecture_simple_service_spec`).

Recent progress (2026-02-03): Architecture root viewport estimation now unions headless service label bounds into the
content bbox (labels are emitted as `<text>` without explicit geometry), per `docs/adr/0057-headless-svg-text-bbox.md`.

Recent progress (2026-02-03): `manatee` FCoSE now matches `cose-base`'s repulsion cutoff behavior
(`repulsionRange = 2 * (level + 1) * idealEdgeLength`) and uses JS-style `Math.floor(Math.random() * upper)` index
selection for spectral sampling. This reduces `parity-root` viewport drift for sparse/disconnected Architecture
fixtures (e.g. `upstream_architecture_docs_service_icon_text` max-width delta shrank from ~+50px to ~+8px).

Recent progress (2026-02-03): Architecture Stage B edges now apply Mermaid's `{group}` and junction endpoint shifts
(`padding + 4`, plus a `+18px` bottom-side label allowance), aligning both geometry and headless viewport estimation
with `packages/mermaid/src/diagrams/architecture/svgDraw.ts` (notably `docs_group_edges`).

Recent progress (2026-02-03): state layout now preserves Mermaid's hidden self-loop helper nodes
(`${nodeId}---${nodeId}---{1|2}`), and the headless SVG viewport approximation now includes Mermaid's `0.1 x 0.1`
placeholder rects to better match upstream `svg.getBBox()` behavior.
Recent progress (2026-02-03): state node sizing now matches Mermaid's rounded `rect` padding behavior (rx/ry ->
`roundedRect`), fixing a common `max-width`/x-offset drift in state diagram root parity.

Recent progress (2026-02-03): `manatee` FCoSE now scales CoSE `minRepulsionDist` with the effective
`idealEdgeLength` (avg / 10) when ideal edge lengths are configured, matching upstream Cytoscape behavior.

Recent progress (2026-02-03): Architecture Stage B now applies a deterministic top-level group separation
post-pass (derived from inter-group edge directions) to approximate Cytoscape compound node spacing and
reduce severe `parity-root` root viewport drift for group-heavy fixtures.
Recent progress (2026-02-03): Architecture top-level group separation now interprets `T/B` edge endpoints
in SVG's y-down coordinate system, fixing the group order inversion observed in `docs_group_edges`.

Recent progress (2026-02-03): `manatee` FCoSE now supports an explicit `defaultEdgeLength` knob (mirroring
`layout-base`'s `DEFAULT_EDGE_LENGTH`) and uses it for repulsion/grid cutoffs and overlap separation buffers,
which makes Architecture root viewport estimation less sensitive to cluster topology.
Recent progress (2026-02-03): Architecture Stage B now approximates `layout-base`'s inter-graph ideal edge
length adjustments (LCA depth factor + group-size-derived additive term) and adds extra separation for Mermaid
`{group}` endpoints, improving `parity-root` deltas for group-heavy fixtures.
Recent progress (2026-02-04): Architecture Stage B now infers missing junction `in_group` membership from
incident non-junction neighbors (unique group or untied top frequency) and derives `{group}` separation gaps
from Mermaid's `architecture.padding` rather than icon size. A tuned extra gap is applied for junction↔junction
edges that also use `{group}` endpoints to better approximate Cytoscape compound repulsion in parity-root mode.
Recent progress (2026-02-04): `manatee::Node` now carries optional `parent` metadata (compound node id) as
groundwork for a future compound-aware FCoSE port (see ADR-0058).

Recent progress (2026-02-02): started a Rust port scaffold of Cytoscape FCoSE in `manatee` (edge
ideal lengths + alignment/relative constraints) and wired it into Architecture headless layout
behind `LayoutOptions.use_manatee_layout` (used by `xtask compare-all-svgs`).
Recent progress (2026-02-03): the `manatee` FCoSE scaffold now applies a CoSE-like repulsion cutoff,
a layout-base-like FR-grid repulsion surrounding cache (refreshed every 10 iterations), and a
range-limited gravity pass on every tick, plus a deterministic collapsed start state for edgeless
graphs to avoid preserving input-grid degeneracy.
Recent progress (2026-02-03): `manatee` FCoSE now applies Mermaid/Cose-base constraints by updating
per-node displacements (a relaxed constraint handling approach) rather than hard-projecting node
positions after each tick. This significantly reduces over-separation in constrained layouts and
brings Architecture `max-width` closer to upstream for several fixtures (e.g. `cypress_split_directioning_normalized`).
Recent progress (2026-02-02): `manatee` FCoSE now includes the upstream spectral initialization
(SVD + power iteration) and uses an explicit seed in place of `Math.random` to keep headless runs
deterministic.

Most parity-root deltas are root `<svg>` viewport attributes (`style` max-width / `viewBox`) and are therefore
sensitive to upstream sizing policy, layout extents (including edge labels/groups), and floating-point rounding.
Recent progress (2026-02-02): Architecture upstream SVG baselines are now generated with a deterministic
browser-side RNG seed (to remove `Math.random()` layout drift in Cytoscape FCoSE); see
`docs/adr/0055-upstream-svg-determinism-for-cytoscape-layouts.md`.
Recent progress (2026-02-02): mindmap headless layout now follows Mermaid node sizing rules (shape/padding/wrapping)
instead of a placeholder grid layout, and `xtask debug-mindmap-svg-positions` was added to compare upstream/local node
coordinates; the remaining parity-root mindmap mismatches are currently dominated by root `<svg>` viewport sizing.
Recent progress (2026-02-04): `manatee` COSE-Bilkent now applies the upstream gravitation pass for disconnected graphs
(`calculateNodesToApplyGravitationTo`), keeping multi-component layouts from drifting too far apart.
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
As of 2026-02-05, `xtask compare-svg-xml --dom-mode strict --dom-decimals 3` reports 175 total strict XML mismatches
(state=43, architecture=25, block=22, class=16, kanban=15, gitgraph=14, mindmap=11, pie=11, xychart=11, c4=7).
Recent progress: gitGraph strict XML compares are now deterministic by seeding auto commit ids (`gitGraph.seed=1`)
in `xtask` and by routing Stage B SVG label measurement through the pipeline `TextMeasurer` (vs an internal
deterministic fallback). Remaining strict mismatches are dominated by CSS/style parity gaps.
Recent progress (2026-02-05): gitGraph `parity-root` root viewport (`viewBox` / `style max-width`) matches upstream by
applying fixture-derived bbox width corrections for branch labels and a couple of auto-generated commit ids.
Recent progress (2026-02-05): state diagram HTML label measurement now matches upstream italic-only `classDef` nodes
(fixes the `Moving` label width and eliminates small Dagre coordinate drift that bubbled into root `viewBox` / `max-width`).
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
