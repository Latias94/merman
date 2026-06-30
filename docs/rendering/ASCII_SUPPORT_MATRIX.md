# ASCII/Unicode Support Matrix

This document describes the user-facing `merman-ascii` support boundary. It is intentionally
stricter than "the parser accepts this Mermaid family": ASCII output is terminal text, so browser-
only styling, rich geometry, and some SVG container semantics may be approximated, summarized, or
omitted.

## Status Levels

| Status | Meaning |
| --- | --- |
| Full | The common Mermaid semantics for this family render as structured terminal diagrams. Styling is still terminal-limited. |
| Partial | Core text/topology semantics render, but important Mermaid presentation or advanced syntax is approximated or omitted. |
| Summary | The renderer intentionally emits readable structured text instead of drawing the full topology. |
| Unsupported | ASCII/Unicode output is not currently exposed for this family. |

`Summary` is still supported output: it is meant for dense or terminal-hostile diagrams where a
faithful box-and-line drawing would be less readable than an explicit relation list.

## Runtime Capability Metadata

The same support story is exposed to bindings through `merman-ascii` capability records
(`crates/merman-ascii/src/capability.rs`) and the `ascii_capabilities_json` binding helper. The
runtime metadata intentionally uses the same status vocabulary as this document and keeps the
legacy `ascii_supported_diagrams` list derived from the richer records.

## Supported Families

| Mermaid family | ASCII status | What renders well | Important limits |
| --- | --- | --- | --- |
| Flowchart / graph | Full | Root directions, boxed nodes, common node shapes, labels, edge labels, open/dotted/thick edges, subgraphs, nested groups, and many Mermaid v11 shape aliases. | Icons, images, callbacks, links, and some browser-only metadata are omitted or unsupported. Some uncommon route shapes are approximate. |
| Sequence | Full | Participants, messages, notes, lifecycle rows, actor boxes, diagram-wide empty boxes, sequence boxes with inner padding, all-participant boxes around dynamic lifecycle content, and Mermaid control blocks including `loop`, `opt`, `break`, `rect`, `alt`, `par`, `par_over`, and `critical`. | Actor presentation metadata and links are omitted. Mirrored bottom participants are opt-in with `--sequence-mirror-actors`; actors destroyed before the footer remain hidden there. |
| State | Partial | States, start/end, transitions, notes, choice/fork/join-like graph nodes, composite groups, class/style colors in ANSI/HTML output. | Some presentation metadata and future state shape variants are terminal approximations. |
| Class | Partial + Summary | Class boxes, members, methods, annotations, notes, interface/lollipop nodes, endpoint labels, common relation markers, self-relation loops, routed chains/stars, bidirectionally scored multi-parent layers, parallel lanes, independent relation components, disconnected standalone components, and dense summary fallback. | Namespace containers are not rendered as nested boxes; namespace-qualified endpoint aliases may be collapsed to local class boxes. Dense/cyclic/collision-prone layouts can use `relations:` summary. Multiple relation markers are unsupported. |
| ER | Partial + Summary | Entity boxes, aliases, attributes, PK/UK/FK tokens, comments, identifying/non-identifying relationships, cardinalities, self-relationship loops, routed chains/stars, bidirectionally scored multi-parent layers, parallel lanes, independent relation components, disconnected standalone components, and dense summary fallback. | Complex cyclic/collision-prone topology may use `relations:` summary. Mermaid CSS styling is preserved in semantic models but only safe terminal colors are represented. Unknown cardinality/relationship kinds are unsupported. |
| XYChart | Partial | Compact bar/line/mixed plots, titles, axes, legends, series labels, terminal value disclosure for data labels, negative values, horizontal/vertical variants, and configurable compact plot areas. | Browser hover tooltips and SVG-coordinate precision are not represented. Dense data is terminal-compact, not pixel-faithful. |
| Gantt | Summary | Titles, sections, tasks, dates, tags, dependencies, and deterministic date formatting. | No terminal timeline geometry; output is a readable task table/summary. |
| GitGraph | Summary | Commits, branches, merges, tags, cherry-picks, and ordering in textual form. | Does not draw a full Git lane graph. |
| Journey | Summary | Sections, tasks, actors, and scores. | Does not draw the Mermaid journey chart geometry. |
| Kanban | Summary | Columns, cards, assignments, and metadata as grouped text. | Drag/drop or board-specific presentation is not represented. |
| Mindmap | Summary | Hierarchical nodes, labels, and nesting as terminal outline/tree text. | Icons, images, and rich node shapes are omitted or approximated. |
| Packet | Full | Bit ranges, labels, row splitting, and multi-row packets. | Visual styling beyond terminal borders is not represented. |
| Timeline | Summary | Sections and events in ordered grouped text. | Does not draw Mermaid timeline geometry. |
| TreeView | Full | Tree nodes, folders/leaves, indentation, and Unicode/ASCII tree connectors. | Browser tree styling is not represented. |
| ZenUML | Partial | Supported ZenUML interactions are translated into sequence-like terminal output, including participants, messages, and basic conditional frames. | The external ZenUML compatibility surface is a subset; unsupported ZenUML syntax is not represented as terminal output. |

## Unsupported Families

These families may parse or render to SVG elsewhere in `merman`, but they are not currently exposed
as ASCII/Unicode render targets:

| Mermaid family | ASCII status | Notes |
| --- | --- | --- |
| Architecture | Unsupported | Rich grouped architecture geometry is SVG-focused. |
| Block | Unsupported | Block layout is SVG-focused. |
| C4 | Unsupported | C4 views remain SVG/headless-render output. |
| Info | Unsupported | Not useful as terminal diagram output today. |
| Pie | Unsupported | No terminal pie/chart approximation yet. |
| Quadrant | Unsupported | No terminal quadrant chart yet. |
| Radar | Unsupported | No terminal radar approximation yet. |
| Requirement | Unsupported | Requirement diagrams are SVG-focused today. |
| Sankey | Unsupported | Flow widths are SVG-specific. |
| Treemap | Unsupported | Rectangle packing is SVG-specific. |

## Playground Filtering

The playground "ASCII supported" filter uses the runtime capability metadata when WASM is ready and
a tracked fallback copy of the same support levels before WASM finishes loading. It still respects
example-level readiness: a family can be generally supported while a specific example is hidden from
the filter if the current ASCII renderer would omit important semantics. For example, basic
`classDiagram` output is supported, but a nested namespace example is not currently ASCII-ready
because namespace containers are not drawn. The preview and export UI show the same full, partial,
or summary support label plus a concise limit for the active diagram type.

## Testing Policy

- Use exact snapshots when the ASCII shape itself is the contract.
- Use semantic assertions for summary fallback, ensuring every entity/endpoint/label remains visible.
- Use explicit `UnsupportedFeature` tests for unsupported semantics instead of silently dropping
  Mermaid input.
- Keep Class/ER dense topology cases on the shared `relation_graph` summary path when routed output
  would overlap boxes or exceed the configured grid budget.
