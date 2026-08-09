# ASCII/Unicode Support Matrix

This document describes the user-facing `merman-ascii` support boundary. It is intentionally
stricter than "the parser accepts this Mermaid family": ASCII output is terminal text, so browser-
only styling, rich geometry, and some SVG container semantics may be approximated, summarized, or
omitted.

## Capability Dimensions

| Field | Values | Meaning |
| --- | --- | --- |
| `semantic_coverage` | `full`, `partial`, or `null` | How much of the family's typed semantics the terminal output preserves. `null` means no output is available. |
| `primary_projection` | `diagrammatic`, `structured_text`, or `none` | Whether the primary output is box-and-line terminal geometry, a readable report/outline, or unavailable. |
| `structured_text_fallback` | Boolean | Whether a diagrammatic family can intentionally fall back to structured text for cases that exceed its readable geometry boundary. |

The legacy `support_level` / `supportLevel` field remains a derived compatibility view:
`none` maps to `unsupported`, `structured_text` maps to `summary`, and `diagrammatic` maps to its
`full` or `partial` semantic coverage. Structured text is supported output, but it is excluded from
diagrammatic-family counts.

## Runtime Capability Metadata

The same support story is exposed to bindings through 31 total `merman-ascii` capability records
(`crates/merman-ascii/src/capability.rs`) and the `ascii_capabilities_json` binding helper. The
legacy `ascii_supported_diagrams` list is derived from records with available output. Product counts
and diagram-only filters use the separate diagrammatic projection.

Binding migration: consumers must replace `summary_fallback` with `structured_text_fallback`, read
`semantic_coverage` and `primary_projection` as the source fields, and treat `support_level` only as
a compatibility view.

## Diagrammatic Families

These families are counted as ASCII diagrams because the primary projection preserves a spatial
relationship: nodes/edges, participants/messages, states/transitions, entities/relations, or plot
coordinates. A family remains `Partial` until its common semantic fields are both retained and
readable at ordinary terminal widths.

| Mermaid family | Semantic coverage | Primary projection | Structured-text fallback | What renders well | Important limits |
| --- | --- | --- | --- | --- | --- |
| Flowchart / graph | Partial | Diagrammatic | No | Root directions, explicit pinned-shape dispositions, common diagrammatic node shapes, independent endpoint markers, normal/dotted/thick/invisible edge semantics, labels, subgraphs, and nested groups. | Icons, images, callbacks, links, some uncommon shapes, declaration-order-independent Dagre ranking, and global dense-route occupancy remain unsupported or incomplete. |
| Sequence | Partial | Diagrammatic | No | Participants, messages, notes, lifecycles, boxes, and Mermaid control blocks. | Actor presentation metadata and links are omitted; mirrored actors are opt-in. |
| State | Partial | Diagrammatic | No | States, transitions, notes, graph-like pseudostates, groups, and terminal colors. | Some presentation metadata and future shape variants are approximated. |
| Class | Partial | Diagrammatic | Yes | Class structure, notes, namespaces, common relations, routed components, and explicit relation summaries. | Namespace-crossing and dense/collision-prone relationships can use `relations:` output. |
| ER | Partial | Diagrammatic | Yes | Entities, attributes, keys, relationship labels/cardinalities, routed components, and explicit relation summaries. | Dense/collision-prone topology can use `relations:` output. |
| XYChart | Partial | Diagrammatic | No | Compact plots, titles, axes, legends, labels, values, and horizontal/vertical variants. | Browser tooltips and SVG coordinate precision are not represented. |
| TreeView | Partial | Diagrammatic | No | Tree nodes, folders/leaves, indentation, and terminal tree connectors. | Typed-field and terminal-usefulness review is incomplete; browser tree styling is not represented. |

## Structured-Text Outputs

These families intentionally produce a terminal-safe semantic report or outline rather than a
diagram. The output is useful for logs, accessibility, debugging, and narrow terminals, but it is
excluded from diagrammatic-family counts and from claims that Merman has an ASCII geometry
implementation. It must not silently substitute for a diagrammatic projection when a resource limit
is exceeded.

| Mermaid family | Semantic coverage | Primary projection | What the report preserves | Why it is not counted as an ASCII diagram |
| --- | --- | --- | --- | --- |
| Gantt | Partial | Structured text | Titles, sections, tasks, resolved dates, tags, and deterministic date formatting. | No timeline geometry; dependency source expressions such as `after task` are not disclosed (A-GANTT-010). |
| GitGraph | Partial | Structured text | Commits, branches, merges, tags, cherry-picks, and ordering. | Does not draw a full Git lane graph. |
| Journey | Partial | Structured text | Sections, tasks, actors, and scores. | Does not draw Mermaid journey geometry. |
| Kanban | Partial | Structured text | Columns, cards, assignments, and metadata. | Drag/drop and board presentation are not represented. |
| Mindmap | Partial | Structured text | Hierarchical nodes and labels as an outline. | Icons, images, and rich node shapes are omitted or approximated. |
| Packet | Partial | Structured text | Bit ranges and labels in ordered terminal rows. | Output does not preserve spatial bit widths; browser-oriented styling is not represented. |
| Timeline | Partial | Structured text | Sections and events in ordered grouped text. | Does not draw Mermaid timeline geometry. |

## Unsupported Families

These families may parse or render to SVG elsewhere in `merman`, but they are not currently exposed
as ASCII/Unicode render targets:

| Mermaid family | ASCII status | Notes |
| --- | --- | --- |
| Architecture | Unsupported | Rich grouped architecture geometry is SVG-focused. |
| Block | Unsupported | Block layout is SVG-focused. |
| C4 | Unsupported | C4 views remain SVG/headless-render output. |
| Cynefin | Unsupported | No terminal projection has been admitted. |
| Event Modeling | Unsupported | No terminal projection has been admitted. |
| Info | Unsupported | Not useful as terminal diagram output today. |
| Ishikawa | Unsupported | No terminal fishbone projection has been admitted. |
| Pie | Unsupported | No terminal pie/chart approximation yet. |
| Quadrant | Unsupported | No terminal quadrant chart yet. |
| Radar | Unsupported | No terminal radar approximation yet. |
| Railroad | Unsupported | No terminal grammar-diagram projection has been admitted. |
| Requirement | Unsupported | Requirement diagrams are SVG-focused today. |
| Sankey | Unsupported | Flow widths are SVG-specific. |
| Treemap | Unsupported | Rectangle packing is SVG-specific. |
| Venn | Unsupported | No terminal set-overlap projection has been admitted. |
| Wardley | Unsupported | No terminal map projection has been admitted. |
| ZenUML | Unsupported | The family has a dedicated typed semantic model and SVG renderer, but no family-owned terminal projection has been admitted. It must not be translated through Sequence as an ASCII shortcut. |

## Playground Filtering

The playground "ASCII supported" filter uses the runtime capability metadata when WASM is ready and
a tracked total fallback copy before WASM finishes loading. It still respects
example-level readiness: a family can be generally supported while a specific example is hidden from
the filter if the current ASCII renderer would omit important semantics. For example, basic
`classDiagram` output is supported, including nested namespace containers. The preview and export
UI show the derived full, partial, or summary label plus a concise limit for the active diagram
type; namespace relationship scenes are still flagged as partial because their relationships render
through an explicit `relations:` summary instead of routed lines through container boxes.

## Testing Policy

- Use exact snapshots when the ASCII shape itself is the contract.
- Use semantic assertions for summary fallback, ensuring every entity/endpoint/label remains visible.
- Use explicit `UnsupportedFeature` tests for unsupported semantics instead of silently dropping
  Mermaid input.
- Keep Class/ER dense topology cases on the shared `relation_graph` summary path when routed output
  would overlap boxes; a resource-limit error remains an error and never becomes a summary.
