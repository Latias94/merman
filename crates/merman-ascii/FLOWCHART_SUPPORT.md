# Flowchart ASCII Support

Status: Active supported subset

This document describes the current `merman-ascii` flowchart support boundary. The renderer consumes
`merman-core` `FlowchartV2Model` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported | `flowchart`, `graph`, and `flowchart-v2` inputs that parse into `FlowchartV2Model`. |
| Directions | Supported subset | `LR`, `TD`, Mermaid's `TB` alias, `BT`, and `RL` root directions. `BT` and `RL` are rendered as terminal-native output transforms of the TD/LR layouts. |
| Node shape | Supported subset | Rectangular shapes, rounded/circle/stadium-like shapes, diamond/decision shapes, subroutine shapes, cylinder/database shapes, hexagon shapes, asymmetric/odd shapes, trapezoid shapes, lean-left/right shapes, datastore shapes, and document shapes. |
| Node labels | Supported subset | Text labels, Mermaid-ascii-compatible escaped newlines, and `<br>` line breaks. Missing labels fall back to node ids. |
| Edges | Supported subset | Directed point arrows, open edges, dotted edges, thick edges, edge labels including multiline labels, deterministic length spacing, and TD same-rank merge edges. |
| Subgraphs | Supported subset | Titled group boxes, multiline title rows from explicit line breaks, automatic wrapping for long titles, nested groups, disconnected sibling groups, external nodes, and boundary-aware cross-boundary routing for the shipped `LR`-inside-`TD` subset. Boundary grid-path labels use planner-owned vertical transit-lane placement and reserve their planned canvas extent instead of being clipped at the original graph width. |
| Layout | Supported subset | LR roots, child levels, multi-root graphs, fan-out/fan-in, self-loops, same-row back edges, crossing/backlink routes, TD branches, and subgraphs use a deterministic grid layout. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| Color roles and styles | Supported subset | Opt-in `AsciiColorMode` can emit ANSI or HTML foreground/background spans for renderer-owned roles and Mermaid flowchart `classDef`, `class`, inline `style`, and `linkStyle` declarations. Supported style properties are `color` for text/labels, `stroke` for borders/edges, and `fill`/`background` for node and subgraph backgrounds. |
| Safety limit | Supported | `AsciiRenderOptions::max_grid_cells` prevents unexpectedly large character grids. |

## V1.1 Compatibility Plan

The next compatibility lane expands high-frequency flowchart constructs with terminal-specific
approximations. These mappings are product behavior once shipped and should be snapshot-tested.

| Capability | Planned behavior | Notes |
| --- | --- | --- |
| Direction transforms | Supported subset. | `BT` vertically flips the TD layout; `RL` horizontally mirrors the LR layout. Node labels, edge labels, group titles, arrowheads, and Unicode connectors stay readable/oriented for the covered root-direction subset. |
| Edge labels | Supported subset. | Labels render on routed edge paths, including multiline labels, for simple LR/TD edges, duplicate LR lanes, LR bidirectional lanes, and TD back-edge lanes. Placement may differ from SVG. |
| Open edges | Supported subset. | Rendered as directionless connectors without arrowheads. |
| Dotted edges | Supported subset. | ASCII uses `.`/`:`; Unicode uses box-drawing dotted line approximations. |
| Thick edges | Supported subset. | ASCII uses `=`/`#` for horizontal/vertical thick lines; Unicode uses heavy box-drawing line characters. |
| Edge length modifiers | Supported subset. | Preserve direction and add deterministic spacing; exact Mermaid rank spacing is not required. |
| Rounded rectangles | Supported approximation. | ASCII uses slash corners; Unicode uses rounded box corners. |
| Circle/double-circle/stadium-like shapes | Supported approximation. | Rendered with the rounded terminal outline; this is not SVG geometry parity. |
| Diamond/decision shapes | Supported approximation. | Rendered with a decision-like terminal outline using `< label >` on the center row. |
| Subroutine shapes | Supported approximation. | Rendered as boxes with inner vertical rails. |
| Cylinder/database shapes | Supported approximation. | Rendered as rounded boxes with an inner top separator. |
| Lean left/right shapes | Supported approximation. | Rendered with mirrored slanted terminal outlines that keep the label centered. |
| Datastore shapes | Supported approximation. | Rendered as a box with blanked side rails to approximate the database barrel. |
| Document shapes | Supported approximation. | Rendered as a box with a folded bottom edge. |
| Subgraphs | Supported subset. | Titled, multiline-title, wrapped-title, nested, disconnected, external-edge, and boundary-aware cross-boundary group layouts for the shipped `LR`-inside-`TD` subset are covered by parser/model tests, local semantic fixtures, and copied `mermaid-ascii` graph fixtures. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Hand-built subgraph member ids with line breaks | `subgraph member ids with line breaks` |
| Hand-built models with directions outside Mermaid's supported root-direction set | `unsupported graph directions` |
| Paper-tape/flag, stacked/tagged document variants, fork/join, icon, image, and other remaining uncommon shapes | `non-rectangular node shapes` |
| Invisible or otherwise non-normal/non-dotted/non-thick strokes | `non-normal edge strokes` |
| Cross, circle, or otherwise non-point edge arrows | `non-point edge arrows` |
| Hand-built models with edges whose endpoints are missing from `nodes` | `edges with missing endpoint nodes` |

## `beautiful-mermaid` Delta Triage

ARI-060 compared the current graph renderer with `repo-ref/beautiful-mermaid/src/ascii/` and
classified the model-expressible deltas below. Mermaid upstream remains the product spec; the
reference implementation is only an implementation aid.

| Delta | Decision | Rationale | Follow-up |
| --- | --- | --- | --- |
| Thick edges | Ported | `merman-core` preserves `edge.stroke = "thick"`, and the existing routing can use alternate line glyphs without changing layout semantics. | Covered by `flowchart_parser_thick_edges_render_with_heavy_ascii_line`, `flowchart_parser_thick_edges_render_with_heavy_unicode_line`, and `flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line`. |
| `BT` root direction | Ported | The typed root direction is available, and honest terminal output is implemented as a post-layout vertical flip with arrow/corner remapping. | Covered by `flowchart_parser_bt_root_direction_renders_with_vertical_flip`. |
| `RL` root direction | Ported with true inversion | `beautiful-mermaid` currently treats `RL` as `LR`, which misrepresents Mermaid semantics; `merman-ascii` implements a true horizontal mirror instead. | Covered by `flowchart_parser_rl_root_direction_renders_with_horizontal_mirror`, `flowchart_parser_rl_multi_character_node_labels_stay_readable`, `flowchart_parser_rl_edge_labels_stay_readable`, and `flowchart_parser_rl_chain_mirrors_unicode_connectors`. |
| Subgraph direction overrides | Ported subset | `FlowSubgraph.dir` now supports canonical local-direction overrides across nested subgraphs, including the boundary-aware cross-boundary cases covered by the current router. The shipped subset covers the common mixed-direction combinations exercised by the nested tests below; broader unsupported Mermaid combinations should still be added explicitly when they are proven. | Covered by `render_model_subgraph_direction_override_renders_local_left_right_layout_without_cross_boundary_edges`, `flowchart_parser_subgraph_direction_override_with_cross_boundary_edges_records_boundary_aware_baseline`, and `flowchart_parser_nested_subgraph_direction_override_keeps_child_group_as_a_movable_block`. |
| Disconnected subgraphs | Ported semantically | `beautiful-mermaid` uses layout-level non-overlap checks for disconnected groups. `merman-ascii` protects the terminal behavior with local semantic assertions: groups remain distinct, content stays visible, and each disconnected LR chain keeps its local row. | Covered by `flowchart_local_semantic_fixture_covers_disconnected_subgraphs`. |
| Multiline and wrapped subgraph labels | Ported | The title text can be represented, and group layout now reserves multiple centered title rows using the shared graph label splitter and display-width wrapper. | Covered by `flowchart_parser_multiline_subgraph_title_renders_centered_rows`, `render_flowchart_renders_model_multiline_subgraph_titles`, and `flowchart_parser_long_subgraph_title_wraps_to_multiple_rows`. |
| ANSI/HTML color roles | Ported | ADR 0067 added an opt-in color API, and flowchart now assigns semantic foreground/background roles after layout. | Covered by `flowchart_color_truecolor_emits_semantic_roles_without_changing_plain_text`, `flowchart_color_html_wraps_subgraph_roles_without_changing_plain_text`, and `flowchart_color_truecolor_preserves_roles_after_horizontal_mirror`. |
| `classDef`, `class`, inline node styles, and `linkStyle` colors | Ported subset | The typed model preserves class/style/linkStyle declarations. The ASCII renderer maps safe terminal semantics: node/subgraph `color` to text/title, node/subgraph `stroke` to borders, node/subgraph `fill`/`background` to ANSI/HTML backgrounds, edge `stroke` to line/arrow foreground, and edge `color` to labels. | Covered by parser-backed `flowchart_style_color_*` tests. |
| State diagram graph rendering | Split to state adapter | `stateDiagram` uses a different typed model, not `FlowchartV2Model`; it now renders through the state-to-graph adapter rather than the flowchart adapter. | See `STATE_SUPPORT.md`. |
| Additional uncommon flowchart shapes | Ported subset | `beautiful-mermaid`'s hexagon, asymmetric, trapezoid, and trapezoid-alt renderers now have terminal approximations in `merman-ascii`. Paper-tape/flag, tagged/stacked document variants, icons, images, and browser-only metadata remain deferred. | Add the remaining shape families one at a time with public `render_model` snapshots. |

## Known Limitations

- LR routing now follows the high-value shape of `mermaid-ascii`'s grid path routing, including
  duplicate and bidirectional label lanes for the supported graph subset.
- TD routing supports vertical chains, branch layouts, same-rank merge edges, bent cross-column
  downward edges, and right-side back-edge label lanes for the copied fixture set.
- `BT` and `RL` remain root-direction transforms only.
- `FlowSubgraph.dir` now ships nested local-direction overrides for the exercised flowchart
  combinations, including the current boundary-aware cross-boundary cases. Add new unsupported
  combinations only when a concrete Mermaid/parser case proves they are still missing.
- Boundary grid-path labels reserve canvas extent and are attached to planner-selected vertical
  transit lanes for the shipped `LR`-inside-`TD` subset. General graph labels still do not have a
  global label layout engine; new dense or ambiguous route families should add explicit route-plan
  policy before admitting complex fixtures.
- Subgraph titles preserve explicit line breaks (`<br>`/escaped newline/model newline) and wrap
  long titles inside the current group box width.
- Mermaid classes/styles are rendered only for terminal-safe color properties in opt-in ANSI/HTML
  modes: `color`, `stroke`, `fill`, and `background` support hex colors and a small named-color
  set. Stroke width, links, callbacks, icons, images, Markdown labels, and HTML labels are not
  rendered.
- CJK/emoji width is measured for box sizing and covered by semantic parser tests for visible node
  and edge labels. Exact spacing from reference multibyte fixtures is intentionally not a
  byte-level oracle.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii graph_golden`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii flowchart`

Golden tests compare against copied `mermaid-ascii` fixtures for the supported subset. The current
graph fixture allowlist covers 79 exact graph matches: 54 ASCII and 25 Unicode.
