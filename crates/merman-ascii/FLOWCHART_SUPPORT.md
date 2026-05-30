# Flowchart ASCII Support

Status: Active supported subset

This document describes the current `merman-ascii` flowchart support boundary. The renderer consumes
`merman-core` `FlowchartV2Model` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported | `flowchart`, `graph`, and `flowchart-v2` inputs that parse into `FlowchartV2Model`. |
| Directions | Supported subset | `LR`, `TD`, Mermaid's `TB` alias, `BT`, and `RL` root directions. `BT` and `RL` are rendered as terminal-native output transforms of the TD/LR layouts. |
| Node shape | Supported subset | Rectangular shapes, rounded/circle/stadium-like shapes, diamond/decision shapes, subroutine shapes, and cylinder/database shapes. |
| Node labels | Supported subset | Text labels, Mermaid-ascii-compatible escaped newlines, and `<br>` line breaks. Missing labels fall back to node ids. |
| Edges | Supported subset | Directed point arrows, open edges, dotted edges, thick edges, edge labels, deterministic length spacing, and `mermaid-ascii` padding directives for simple LR/TD edges. |
| Subgraphs | Supported subset | Titled group boxes, nested groups, external nodes, and subgraph edge crossings covered by copied `mermaid-ascii` graph fixtures. |
| Layout | Supported subset | LR roots, child levels, multi-root graphs, fan-out/fan-in, self-loops, same-row back edges, crossing/backlink routes, TD branches, and subgraphs use a deterministic grid layout. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| Safety limit | Supported | `AsciiRenderOptions::max_grid_cells` prevents unexpectedly large character grids. |

## V1.1 Compatibility Plan

The next compatibility lane expands high-frequency flowchart constructs with terminal-specific
approximations. These mappings are product behavior once shipped and should be snapshot-tested.

| Capability | Planned behavior | Notes |
| --- | --- | --- |
| Direction transforms | Supported subset. | `BT` vertically flips the TD layout; `RL` horizontally mirrors the LR layout. Node labels, edge labels, group titles, arrowheads, and Unicode connectors stay readable/oriented for the covered root-direction subset. |
| Edge labels | Supported subset. | Labels render on routed edge paths for simple LR/TD edges, duplicate LR lanes, LR bidirectional lanes, and TD back-edge lanes. Placement may differ from SVG. |
| Open edges | Supported subset. | Rendered as directionless connectors without arrowheads. |
| Dotted edges | Supported subset. | ASCII uses `.`/`:`; Unicode uses box-drawing dotted line approximations. |
| Thick edges | Supported subset. | ASCII uses `=`/`#` for horizontal/vertical thick lines; Unicode uses heavy box-drawing line characters. |
| Edge length modifiers | Supported subset. | Preserve direction and add deterministic spacing; exact Mermaid rank spacing is not required. |
| Rounded rectangles | Supported approximation. | ASCII uses slash corners; Unicode uses rounded box corners. |
| Circle/double-circle/stadium-like shapes | Supported approximation. | Rendered with the rounded terminal outline; this is not SVG geometry parity. |
| Diamond/decision shapes | Supported approximation. | Rendered with a decision-like terminal outline using `< label >` on the center row. |
| Subroutine shapes | Supported approximation. | Rendered as boxes with inner vertical rails. |
| Cylinder/database shapes | Supported approximation. | Rendered as rounded boxes with an inner top separator. |
| Subgraphs | Supported subset. | Titled, nested, and external-edge group layouts covered by copied `mermaid-ascii` graph fixtures render exactly. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Multiline subgraph labels | `multiline subgraph labels` |
| Multiline edge labels | `multiline edge labels` |
| Hand-built models with directions outside Mermaid's supported root-direction set | `unsupported graph directions` |
| Hexagon, lean, document, fork/join, icon, image, and other uncommon shapes | `non-rectangular node shapes` |
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
| Subgraph direction overrides | Defer | `FlowSubgraph.dir` is typed, but current graph layout is global; local subgraph layout needs a deeper layout pass. | Split with nested subgraph and cross-boundary edge fixtures. |
| Multiline subgraph labels | Defer | The title text can be represented, but group layout reserves a single title row. | Expand group header height and route edges around taller headers. |
| ANSI/HTML color roles | Defer | The current public options intentionally produce plain ASCII/Unicode text and have no role canvas or color mode. | Design an opt-in color API before emitting escape sequences or HTML spans. |
| `classDef`, `class`, and inline node styles | Defer | Style data exists in the typed model, but rendering it depends on the color-role API decision. | Revisit together with ANSI/HTML color roles. |
| State diagram graph rendering | Defer/split | `stateDiagram` uses a different typed model, not `FlowchartV2Model`; adapting it through graph rendering needs a state-to-graph semantic adapter. | Open a state ASCII workstream if prioritized. |
| Additional uncommon flowchart shapes | Defer | `beautiful-mermaid` has more shape renderers; current `merman-ascii` intentionally supports the high-frequency terminal approximations first. | Add one shape family at a time with public `render_model` snapshots. |

## Known Limitations

- LR routing now follows the high-value shape of `mermaid-ascii`'s grid path routing, including
  duplicate and bidirectional label lanes for the supported graph subset.
- TD routing supports vertical chains, branch layouts, bent cross-column downward edges, and
  right-side back-edge label lanes for the copied fixture set.
- `BT` and `RL` are root-direction transforms only. `FlowSubgraph.dir` overrides remain a separate
  layout follow-on.
- Leading `paddingX=` and `paddingY=` lines are supported as `mermaid-ascii` compatibility
  directives by ASCII render entry points; they are not Mermaid flowchart syntax.
- Classes, styles, links, callbacks, icons, images, Markdown labels, and HTML labels are not rendered.
- CJK/emoji width is measured for box sizing, but full multi-cell text placement needs dedicated
  follow-up coverage before being listed as supported.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii graph_golden`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii flowchart`

Golden tests compare against copied `mermaid-ascii` fixtures for the supported subset. The current
graph fixture allowlist covers 75 exact graph matches: 52 ASCII and 23 Unicode.
