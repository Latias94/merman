# Flowchart ASCII Support

Status: ASCII graph routing parity expansion in progress

This document describes the current `merman-ascii` flowchart support boundary. The renderer consumes
`merman-core` `FlowchartV2Model` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported | `flowchart`, `graph`, and `flowchart-v2` inputs that parse into `FlowchartV2Model`. |
| Directions | Supported subset | `LR`, `TD`, and Mermaid's `TB` alias. |
| Node shape | Supported subset | Rectangular shapes, rounded/circle/stadium-like shapes, diamond/decision shapes, subroutine shapes, and cylinder/database shapes. |
| Node labels | Supported subset | Single-line text labels. Missing labels fall back to node ids. |
| Edges | Supported subset | Directed point arrows, open edges, dotted edges, edge labels, and deterministic length spacing for simple LR/TD edges. |
| Subgraphs | Supported subset | Simple titled group boxes render around supported member nodes with upstream-style title rows. |
| Layout | Supported subset | LR roots, child levels, multi-root graphs, basic fan-out/fan-in, self-loops, same-row back edges, and simple subgraphs use a deterministic grid layout. TD remains a simpler vertical layout. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| Safety limit | Supported | `AsciiRenderOptions::max_grid_cells` prevents unexpectedly large character grids. |

## V1.1 Compatibility Plan

The next compatibility lane expands high-frequency flowchart constructs with terminal-specific
approximations. These mappings are product behavior once shipped and should be snapshot-tested.

| Capability | Planned behavior | Notes |
| --- | --- | --- |
| Edge labels | Supported subset. | Labels render near the edge path for simple LR/TD edges. Placement may differ from SVG. |
| Open edges | Supported subset. | Rendered as directionless connectors without arrowheads. |
| Dotted edges | Supported subset. | ASCII uses `.`/`:`; Unicode uses box-drawing dotted line approximations. |
| Edge length modifiers | Supported subset. | Preserve direction and add deterministic spacing; exact Mermaid rank spacing is not required. |
| Rounded rectangles | Supported approximation. | ASCII uses slash corners; Unicode uses rounded box corners. |
| Circle/double-circle/stadium-like shapes | Supported approximation. | Rendered with the rounded terminal outline; this is not SVG geometry parity. |
| Diamond/decision shapes | Supported approximation. | Rendered with a decision-like terminal outline using `< label >` on the center row. |
| Subroutine shapes | Supported approximation. | Rendered as boxes with inner vertical rails. |
| Cylinder/database shapes | Supported approximation. | Rendered as rounded boxes with an inner top separator. |
| Subgraphs | Supported subset. | Simple titled group boxes render around supported member nodes; complex nested/external-edge routing remains a follow-on. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Multiline subgraph labels | `multiline subgraph labels` |
| Multiline edge labels | `multiline edge labels` |
| `BT`, `RL`, or other non-LR/TD directions | `non-LR/TD graph directions` |
| Hexagon, lean, document, fork/join, icon, image, and other uncommon shapes | `non-rectangular node shapes` |
| Multiline node labels | `multiline node labels` |
| Thick, invisible, or otherwise non-normal/non-dotted strokes | `non-normal edge strokes` |
| Cross, circle, or otherwise non-point edge arrows | `non-point edge arrows` |
| Hand-built models with edges whose endpoints are missing from `nodes` | `edges with missing endpoint nodes` |

## Known Limitations

- The current layout is a growing grid-based parity implementation, not the full upstream routing
  algorithm.
- Crossing junction merging, duplicate/bidirectional label separation, and TD back-edge labels are
  not product-supported yet.
- Complex nested subgraph routing and external-edge routing through subgraphs are not product-
  supported yet.
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
graph fixture allowlist covers 44 exact graph matches: 26 ASCII and 18 Unicode.
