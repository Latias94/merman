# Flowchart ASCII Support

Status: Active supported subset

This document describes the current `merman-ascii` flowchart support boundary. The renderer consumes
`merman-model` Flowchart terminal views; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported | `flowchart`, `graph`, and `flowchart-v2` inputs that parse into `FlowchartModel`. |
| Directions | Supported subset | `LR`, `TD`, Mermaid's `TB` alias, `BT`, and `RL` root directions. `BT` and `RL` are rendered as terminal-native output transforms of the TD/LR layouts. |
| Node shape | Supported subset | Every pinned Mermaid shape name has one explicit terminal disposition. Common rectangular/process, rounded, circle/double-circle/stadium, diamond/decision, subroutine, cylinder/database, hexagon, asymmetric, trapezoid, lean-left/right, datastore, document/decorated-document, decorated-process, fork/join, choice, start/end, and text shapes have diagrammatic projections. Manual-input, stored-data/bow-tie, and flag/paper-tape alias families have explicit terminal approximations that follow the pinned upstream handler groups. Card, hourglass, communication link, left/right/paired comment, delay, disk/direct-access/internal storage, display, divided process, extract, loop limit, manual file, summary, cloud, and icon shapes use measured rectangular compartments with visible type cues. Unknown, cross-family, BPMN, and remaining rich object shapes fail explicitly. |
| Icon and image metadata | Textual approximation | Complete authored labels remain visible. Icon/image identifiers, form, label position, constraint, and asset dimensions are quoted or numeric details inside the node. These rows wrap once, preserve quoted whitespace, and consume space before contour sizing and routing. Assets are not fetched, glyphs or pixels are not reproduced, and source pixel dimensions do not determine terminal box dimensions. Fixed control shapes reject asset metadata rather than hiding it. |
| Node labels | Supported subset | Text labels, Mermaid-ascii-compatible escaped newlines, and `<br>` line breaks. Source-preserving labels decode text entities once after authored breaks are identified; entity-created `<br>` and `\n` stay literal. Markdown emphasis and strong delimiters are removed through parser-owned source ranges for nodes, edges, and groups. Plain-label punctuation remains literal; code, links, and other Markdown retain their source notation. Ordinary Flowchart node labels wrap before node sizing at the configurable `flowchart_node_label_wrap_width` terminal-cell limit (canonical default `40`; Compact default `24` unless explicitly overridden), using one grapheme-safe plan for measurement and materialization. Missing labels fall back to node ids. This terminal policy is independent of Mermaid's CSS-pixel `wrappingWidth`. |
| Edges | Supported subset | Independent source/target point, circle, cross, or open markers; mixed and double-ended forms; visible normal/dotted/thick strokes; invisible layout constraints; explicit/generated edge identity; multiline labels; deterministic length spacing; and TD merge/skip edges. Endpoint markers remain unresolved until scene allocation assigns each owner an independent terminal berth. Labels keep their planned footprint when clear and otherwise use bounded route-local lanes that avoid endpoint markers, nodes, group borders/titles, and accepted labels. Clear spans use direct ports; obstructed spans use family-owned alternate lanes. |
| Subgraphs | Supported subset | Titled group boxes, multiline title rows from explicit line breaks, automatic wrapping for long titles, nested groups, disconnected sibling groups, external nodes, authored local directions across boundary edges, and the pinned Mermaid baseline's perpendicular default for isolated implicit subgraphs. Boundary grid-path labels use planner-owned vertical transit-lane placement and reserve their planned canvas extent instead of being clipped at the original graph width. |
| Layout profiles | Supported subset | `canonical` remains the default. The explicit `compact` profile keeps the same topology and routing semantics while resolving the family-owned node-label wrap default from 40 to 24 cells and horizontal rank gap from 5 to 3. Caller overrides remain authoritative. Flowchart and Swimlane share these profiles; State remains Canonical-only. |
| Cyclic rank recovery | Supported subset | After a routing miss with no further endpoint, corridor, or return-lane growth, flat visible-edge scenes without authored side constraints may place each strongly connected component on one main-axis level. All member nodes and original relationships remain independently rendered; cross-component lengths constrain the condensation DAG. The candidate shares the existing finite rebuild/work allowances and must pass normal scene validation. Initial and capacity-recovered successes retain their geometry; compound and invisible-constraint graphs retain existing rules. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| Color roles and styles | Supported subset | Plain, ANSI16, ANSI256, TrueColor, and HTML encoders share one layout. ANSI16 keeps primary text at terminal Reset and uses sparse named accents for renderer-owned structure. Mermaid flowchart `classDef`, `class`, inline `style`, and `linkStyle` declarations can supply supported foreground/background roles. Supported style properties are `color` for text/labels, `stroke` for borders/edges, and `fill`/`background` for node and subgraph backgrounds. |
| Viewport/report behavior | Supported | Allow preserves complete wide output, Error returns a typed width diagnostic, and Plain Fallback selects one complete typed structured projection. Styled fallback is rejected by capability preflight. Schema-3 output metadata identifies encoding, requested/effective layout profiles and Compact attempts; the final line terminator does not count as a logical row. |
| Resource policy | Supported | Typed limits bound checked grid extent, layout work, logical document cells, encoded bytes, grapheme size, and nesting depth before amplification. |

## V1.1 Compatibility Plan

The next compatibility lane expands high-frequency flowchart constructs with terminal-specific
approximations. These mappings are product behavior once shipped and should be snapshot-tested.

| Capability | Planned behavior | Notes |
| --- | --- | --- |
| Direction transforms | Supported subset. | `BT` vertically flips the TD layout; `RL` horizontally mirrors the LR layout. Node labels, edge labels, group titles, arrowheads, and Unicode connectors stay readable/oriented for the covered root-direction subset. |
| Edge labels | Supported subset. | Labels render on routed edge paths, including multiline labels, for simple LR/TD edges, duplicate LR lanes, LR bidirectional lanes, and TD back-edge lanes. Grid and feedback planners retain the complete straight host so an inline label can cover its own segment. Relocated labels keep a clear perpendicular gap to that host; the gap is reserved against later routes and labels. Multiline side lanes align the complete text block, and labeled self-loops reserve a measured envelope with visible connections around the text. Placement may differ from SVG. The scene allocator keeps the original placement when clear, probes bounded route-local side lanes on collision, and returns an explicit error only when every non-destructive candidate is exhausted. |
| Open edges | Supported subset. | Rendered as directionless connectors without arrowheads. |
| Dotted edges | Supported subset. | ASCII uses `.`/`:`; Unicode uses box-drawing dotted line approximations. |
| Thick edges | Supported subset. | ASCII uses `=`/`#` for horizontal/vertical thick lines; Unicode uses heavy box-drawing line characters. |
| Independent endpoint markers | Supported subset. | Point, circle, cross, and open source/target markers remain independently typed for full and labeled edge forms. ASCII and Unicode use terminal-native marker glyphs. |
| Invisible edges | Supported. | Invisible Mermaid edges participate in graph layout but do not consume visible route lanes or paint cells. |
| Edge length modifiers | Supported subset. | Preserve direction and add deterministic spacing; exact Mermaid rank spacing is not required. |
| Rounded rectangles | Supported approximation. | ASCII uses slash corners; Unicode uses rounded box corners. |
| Circle/double-circle/stadium-like shapes | Supported approximation. | Rendered with the rounded terminal outline; this is not SVG geometry parity. |
| Diamond/decision shapes | Supported approximation. | Rendered with a decision-like terminal outline using `< label >` on the center row. |
| Subroutine shapes | Supported approximation. | Rendered as boxes with inner vertical rails. |
| Cylinder/database shapes | Supported approximation. | Rendered as rounded boxes with an inner top separator. |
| Lean left/right shapes | Supported approximation. | Rendered with mirrored slanted terminal outlines that keep the label centered. |
| Manual-input shapes | Supported approximation. | `sl-rect`, `manual-input`, and `sloped-rectangle` share one sloped-top terminal projection, matching the pinned upstream alias handler. |
| Datastore shapes | Supported approximation. | Rendered as a box with blanked side rails to approximate the database barrel. |
| Stored-data shapes | Supported approximation. | `bow-rect`, `stored-data`, and `bow-tie-rectangle` share one concave-sided terminal projection instead of collapsing into datastore. |
| Paper-tape shapes | Supported approximation. | `flag` and `paper-tape` share one wavy-edge projection because Mermaid assigns both names to the same handler. |
| Document shapes | Supported approximation. | Rendered as a box with a folded bottom edge. |
| Subgraphs | Supported subset. | Titled, multiline-title, wrapped-title, nested, disconnected, external-edge, and boundary-aware cross-boundary group layouts are covered by parser/model tests and local semantic fixtures. Copied `mermaid-ascii` fixtures remain immutable; implicit isolated groups intentionally differ where the narrow renderer inherits the root axis instead of the pinned Mermaid baseline's perpendicular default. |

With the default `AsciiGraphRoutingSpace::Expand`, preferred graph gaps can grow to fit a retained
edge label's complete height, endpoint markers, and visible connections. Blank edge labels are
removed before reserving label space.

For a Flowchart without subgraphs, `AsciiGraphRoutingSpace::Fixed` keeps node geometry and rank
spacing at the resolved sizes, including the shape and the minimum cells for visible strokes and
markers. Labels and detours still contribute to the emitted extent. Fixed requires Strict
presentation. The compact cylinder uses its two inner side cells to show the lid when the first
interior row is occupied by text. A host can retry with Expand after a typed routing miss. The
issue #370 diagram renders in 33 rows with Fixed and 48 rows with Expand at a two-cell rank gap
and zero vertical node padding. [#370](https://github.com/Latias94/merman/issues/370)

Labeled reciprocal parallel edges retain independent target heads. Opposite-directed edges may
cross perpendicularly while keeping their collinear transit strokes separate.

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Hand-built subgraph member ids with line breaks | `subgraph member ids with line breaks` |
| Hand-built models with directions outside Mermaid's supported root-direction set | `unsupported graph directions` |
| Known cross-family, BPMN, and remaining rich object shapes | `unsupported flowchart node shape projections` |
| Unknown shape names in detached typed models | `unknown flowchart node shapes` |
| Asset metadata on fixed-label start/end/choice/fork/join control shapes | `metadata on fixed control node shapes` |
| Hand-built models with edges whose endpoints are missing from `nodes` | `edges with missing endpoint nodes` |

Required route or label search exhaustion returns `AsciiError::RoutingSearchExhausted` with the
authored edge index. Independent marker demand above the current fixed presentation's capacity
upper bound returns `AsciiError::PortCapacityExceeded`. These diagnostics do not establish that
other layouts are impossible and do not trigger structured width fallback. See the
[graph presentation boundary](../../../crates/merman-ascii/README.md#graph-presentation-boundary).

## `beautiful-mermaid` Delta Triage

ARI-060 compared the current graph renderer with `repo-ref/beautiful-mermaid/src/ascii/` and
classified the model-expressible deltas below. Mermaid upstream remains the product spec; the
reference implementation is only an implementation aid.

| Delta | Decision | Rationale | Follow-up |
| --- | --- | --- | --- |
| Thick edges | Ported | `merman-core` preserves `edge.stroke = "thick"`, and the existing routing can use alternate line glyphs without changing layout semantics. | Covered by `flowchart_parser_thick_edges_render_with_heavy_ascii_line`, `flowchart_parser_thick_edges_render_with_heavy_unicode_line`, and `flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line`. |
| `BT` root direction | Ported | The typed root direction is available, and honest terminal output is implemented as a post-layout vertical flip with arrow/corner remapping. | Covered by `flowchart_parser_bt_root_direction_renders_with_vertical_flip`. |
| `RL` root direction | Ported with true inversion | `beautiful-mermaid` currently treats `RL` as `LR`, which misrepresents Mermaid semantics; `merman-ascii` implements a true horizontal mirror instead. | Covered by `flowchart_parser_rl_root_direction_renders_with_horizontal_mirror`, `flowchart_parser_rl_multi_character_node_labels_stay_readable`, `flowchart_parser_rl_edge_labels_stay_readable`, and `flowchart_parser_rl_chain_mirrors_unicode_connectors`. |
| Subgraph direction overrides | Ported subset | `FlowSubgraph.dir` supports canonical local-direction overrides across nested subgraphs and preserves authored directions across external boundary edges. Isolated subgraphs without an explicit direction use the pinned Mermaid baseline's perpendicular parent-axis default. | Covered by `render_model_subgraph_direction_override_renders_local_left_right_layout_without_cross_boundary_edges`, `flowchart_parser_explicit_subgraph_direction_survives_cross_boundary_edges`, `flowchart_parser_sibling_groups_keep_explicit_directions_across_external_edges`, `flowchart_parser_isolated_implicit_subgraph_uses_mermaid_perpendicular_default`, and `flowchart_parser_nested_subgraph_direction_override_keeps_child_group_as_a_movable_block`. |
| Disconnected subgraphs | Ported semantically | `beautiful-mermaid` uses layout-level non-overlap checks for disconnected groups. `merman-ascii` protects the terminal behavior with local semantic assertions: groups remain distinct, content stays visible, and each isolated implicit group follows the perpendicular parent-axis default. | Covered by `flowchart_local_semantic_fixture_covers_disconnected_subgraphs`. |
| Multiline and wrapped subgraph labels | Ported | The title text can be represented, and group layout now reserves multiple centered title rows using the shared graph label splitter and display-width wrapper. | Covered by `flowchart_parser_multiline_subgraph_title_renders_centered_rows`, `render_flowchart_renders_model_multiline_subgraph_titles`, and `flowchart_parser_long_subgraph_title_wraps_to_multiple_rows`. |
| Long ordinary node labels | Ported with terminal-native sizing | Issue #53 exposes a real terminal readability problem, but SVG pixel measurements are not a stable terminal unit. Flowchart therefore applies a configurable display-cell width before shape sizing and routing, preserving hard breaks and grapheme ownership. | Covered by `flowchart_issue_53_long_node_labels_wrap_before_layout`, `flowchart_node_label_wrap_width_changes_the_pre_layout_plan`, and `flowchart_node_label_wrapping_preserves_authored_breaks_and_graphemes`. |
| ANSI/HTML color roles | Ported | ADR 0067 added an opt-in color API, and flowchart now assigns semantic foreground/background roles after layout. | Covered by `flowchart_color_truecolor_emits_semantic_roles_without_changing_plain_text`, `flowchart_color_html_wraps_subgraph_roles_without_changing_plain_text`, and `flowchart_color_truecolor_preserves_roles_after_horizontal_mirror`. |
| `classDef`, `class`, inline node styles, and `linkStyle` colors | Ported subset | The typed model preserves class/style/linkStyle declarations. The ASCII renderer maps safe terminal semantics: node/subgraph `color` to text/title, node/subgraph `stroke` to borders, node/subgraph `fill`/`background` to ANSI/HTML backgrounds, edge `stroke` to line/arrow foreground, and edge `color` to labels. | Covered by parser-backed `flowchart_style_color_*` tests. |
| State diagram graph rendering | Split to state adapter | `stateDiagram` uses a different typed model, not `FlowchartModel`; it now renders through the state-to-graph adapter rather than the flowchart adapter. | See `STATE_SUPPORT.md`. |
| Additional uncommon flowchart shapes | Ported subset | One projection registry assigns every pinned Mermaid shape name an implemented, approximate, annotated, or unsupported terminal disposition. Known annotated families retain a visible role in a measured compartment instead of silently becoming ordinary rectangles. Icon/image metadata uses complete terminal-safe text. Cross-family boxes, BPMN, and remaining rich object shapes remain explicit errors. | Parser-backed alias, asset identity, route/label, terminal safety, and cell-alignment regressions in `crates/merman-ascii/tests/flowchart_model/shapes.rs`; projection resource and output contracts. |

## Known Limitations

- Route candidates retain all cardinal port combinations. Collision or congestion can trigger
  terminal-cell search over every shape-valid non-corner contact on each node side, preserving
  independent heads for covered merges through twelve incoming edges, branch/merge graphs, skip
  edges, and mixed-stroke feedback. Node-to-node routes across nested subgraphs use the same containment and transverse
  crossing constraints as coarse-grid recovery. Aligned edges blocked by intermediate nodes reserve
  a bypass lane in their nearest common container; group-title footprints include a horizontal
  reading gap where space permits. Explicit local directions remain authoritative. Container
  endpoints participate in terminal-cell recovery with distinct exterior and ancestor/member
  interior contacts; only selected endpoint contacts may touch that frame, excluding corners.
  Container self-loops can use two distinct exterior contacts, including labeled parallel loops
  beside an exit. Nested loops and their labels stay inside their common parent, which reserves
  enough exterior space to leave a berth before turning. Parent layouts account for complete
  child-content spans;
  sibling subgraphs remain separate units during local-direction placement. Clear direct routes
  remain direct, and Dagre ranks do not change. Where a globally empty separator exists, branch
  spacing reserves a complete transit band beyond endpoint reading margins on the actual local
  rank axis without expanding any node's content bands. Source markers participate in port-reuse
  scoring; open starts may share their stems. Actual markers and primary terminals remain
  protected, while unused alternate marker positions allow transverse crossings. Marker allocation
  rejects those positions after a crossing takes them. Covered two-ended-marker fans through fourteen
  edges retain independent, unstacked heads in every root direction, including at zero graph padding.
  If the top or left canvas boundary hides contacts needed by marked endpoint demand, layout
  reserves an exterior reading margin and turn cell by translating the entire scene, including
  compound frames, divider spans, and the coarse-grid origin. Node sizes and relative ranks stay
  unchanged. Endpoints with multiple markers also include the escape corridors of their unmarked
  ends when reserving that space. Marker admission rejects adjacent pointing-axis heads belonging
  to different branches at one endpoint; same-pair parallel marker chains keep distinct owners.
  Optional quality repair has a fixed expansion cap across lazily generated port pairs; required
  recovery is bounded by scene extent and the shared work ledger. Full contour enumeration does
  not guarantee independent routes for arbitrary degree. Dense two-ended-marker fan-out can still
  exhaust recovery or the work budget; sixteen-edge TD/BT cases can exhaust the Interactive work
  budget at default and zero preferred graph padding.
  Alternate candidates and recovery still run
  when the primary route family is unavailable. Candidate exhaustion produces `RoutingSearchExhausted`;
  resource and cancellation failures remain terminal errors rather than a partial drawing.
- LR routing now follows the high-value shape of `mermaid-ascii`'s grid path routing, including
  duplicate and bidirectional label lanes for the supported graph subset.
- TD routing supports vertical chains, branch layouts, same-rank merge/skip edges in either
  horizontal direction, bent cross-column downward edges, and right-side back-edge label lanes for
  the copied fixture set. Same-rank spans use direct horizontal ports when clear and when any
  label fits without covering an arrowhead; otherwise they use the shared bottom lane.
- Endpoint markers are independently allocated even for same-glyph/same-color owners. Edge labels
  are assigned and protected when their route is committed, so later routes cannot consume an
  earlier label's berth. Parallel node self-loops reserve nested lane space before node projection
  and allocate their labels together after the batch's return lanes are known and before unrelated
  edges or a searched loop are planned. Blocked node loops can use alternate terminal ports;
  earlier labels are reserved before that search and its incumbent is re-admitted against them.
  Searched-loop and container-loop labels are committed before the next loop's search, so later
  contacts and transit cells respect their footprints. Terminal-cell paths try their straight
  segments as label hosts in stable length order. A boundary route without a vertical transit
  segment keeps its label on an inline
  host instead of dropping it. Transverse crossings keep both axes visible: Unicode `╪`
  denotes an independent overpass, while `┼` denotes a joined four-way channel. ASCII/CJK
  structure uses `+` for both and cannot make that distinction in one cell. The crossing cell
  is neutral to stroke weight; adjacent cells retain normal/dotted/thick identity, and the
  horizontal route supplies its color. Incompatible mixed-stroke overlaps must use another lane.
  Multiline label rows share one integer host column even when their terminal widths have
  different parity. Reverse-direction output mirrors their padding while preserving text order.
  Labels respect endpoint-container ownership, shared ancestor interiors, and planned marker
  berths. Independent vertical stems and vertical markers retain a column of reading clearance
  from neighboring labels, regardless of which relationship is planned first. A label can still
  use its own vertical host or inline horizontal stroke.
  External group labels cannot leak into member space. Single-cell terminal paths require
  at most one visible endpoint marker; dual-marker edges try another port pair. Arbitrary dense
  routing and non-transverse mixed-stroke junctions remain outside the admitted boundary.
- Node ranks use the shared Dagre-compatible acyclic/nesting/ranker phases and retain `minlen`,
  cycle ownership, compound constraints, and root direction. Local subgraph direction transforms
  reapply forward cross-boundary rank constraints after moving the group block.
- Coarse external-node separation uses member envelopes, while axis projection resolves terminal
  frame padding. These coordinate systems are not interchangeable; repeated terminal-padding
  expansion on the coarse grid previously produced large blank offsets for group endpoint edges.
- `BT` and `RL` remain root-direction transforms only.
- `FlowSubgraph.dir` ships nested local-direction overrides across boundary edges. Isolated
  subgraphs without an explicit direction use the pinned Mermaid baseline's perpendicular effective-parent
  axis. Add new unsupported combinations only when a concrete Mermaid/parser case proves they are
  still missing.
- Boundary grid-path labels reserve canvas extent and are attached to planner-selected vertical
  transit lanes for the shipped `LR`-inside-`TD` subset. General labels share protected scene
  footprints, while dense route families still need explicit route-candidate policy before new
  complex fixtures are admitted.
- Subgraph titles preserve explicit line breaks (`<br>`/escaped newline/model newline) and wrap
  long titles inside the current group box width.
- Ordinary Flowchart node labels wrap at `flowchart_node_label_wrap_width` display cells before
  shape geometry and ports are computed. This is an ASCII terminal policy, not a conversion of
  Mermaid's SVG `wrappingWidth` pixels. State labels and Flowchart edge labels retain their own
  family policies.
- Compact is a Flowchart/Swimlane-owned admission, not a global graph-density mutation. It changes
  the unresolved ordinary-node wrap default from 40 to 24 display cells and the horizontal rank gap
  from 5 to 3; explicit spacing/wrap builders win, and State remains Canonical-only.
- Mermaid classes/styles are rendered only for terminal-safe color properties in opt-in ANSI/HTML
  modes: `color`, `stroke`, `fill`, and `background` support hex colors and a small named-color
  set. Stroke width, links, and callbacks are not rendered. Icons and images use the textual
  projection above. Label markup follows the separate terminal label rules above.
- CJK/emoji width is measured for box sizing and covered by semantic parser tests for visible node
  and edge labels. Exact spacing from reference multibyte fixtures is intentionally not a
  byte-level oracle.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii --all-features -j 2`
- `cargo nextest run -p merman-ascii --features diagram-flowchart --test presentation_floor -j 2`

Golden tests compare against immutable copied `mermaid-ascii` fixtures for the supported subset. The
current corpus has 79 graph fixtures: 28 exact upstream matches, seven strict corrected compound
snapshots, two strict local wire-style snapshots, and 42 named deterministic layout/route differences.
The [fixture inventory](../../../crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md) owns these dispositions;
all cases must render and preserve their admitted text and parser-backed semantic contracts.
The viewport characterization matrix additionally exercises canonical/compact, ASCII/Unicode,
Unicode/CJK width profiles, Plain/ANSI16/ANSI256/TrueColor/HTML geometry parity, and
60/80/100/120-cell Allow/Fallback/Error outcomes over the Issue #53 fixture.
