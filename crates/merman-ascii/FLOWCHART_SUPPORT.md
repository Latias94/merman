# Flowchart ASCII Support

Status: initial tracer-bullet support

This document describes the current `merman-ascii` flowchart support boundary. The renderer consumes
`merman-core` `FlowchartV2Model` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported | `flowchart`, `graph`, and `flowchart-v2` inputs that parse into `FlowchartV2Model`. |
| Directions | Supported subset | `LR`, `TD`, and Mermaid's `TB` alias. |
| Node shape | Supported subset | Rectangular parser shapes: `squareRect`, `rect`, `rectangle`, and `square`. |
| Node labels | Supported subset | Single-line text labels. Missing labels fall back to node ids. |
| Edges | Supported subset | Directed point arrows with normal stroke and length `1`. |
| Layout | Supported subset | Linear node order using the model's node order. |
| Character sets | Supported | ASCII and Unicode box-drawing output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| Safety limit | Supported | `AsciiRenderOptions::max_grid_cells` prevents unexpectedly large character grids. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Subgraphs/clusters | `subgraphs` |
| Edge labels | `edge labels` |
| `BT`, `RL`, or other non-LR/TD directions | `non-LR/TD graph directions` |
| Non-rectangular node shapes | `non-rectangular node shapes` |
| Multiline node labels | `multiline node labels` |
| Link length modifiers | `edge length modifiers` |
| Dotted, thick, or otherwise non-normal strokes | `non-normal edge strokes` |
| Open, cross, circle, or otherwise non-point edge arrows | `non-point edge arrows` |
| Hand-built models with edges whose endpoints are missing from `nodes` | `edges with missing endpoint nodes` |

## Known Limitations

- The current layout is a tracer-bullet linear layout, not the full upstream routing algorithm.
- Multi-root graphs, branches, back-edges, and non-adjacent routing are not product-supported yet.
- Classes, styles, links, callbacks, icons, images, Markdown labels, and HTML labels are not rendered.
- CJK/emoji width is measured for box sizing, but full multi-cell text placement needs dedicated
  follow-up coverage before being listed as supported.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii graph_golden`
- `cargo nextest run -p merman-ascii flowchart`

Golden tests compare against copied `mermaid-ascii` fixtures for the initial supported subset.
