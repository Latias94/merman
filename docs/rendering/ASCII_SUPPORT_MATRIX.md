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
| `structured_text_fallback` | Boolean | Whether the typed family has an admitted complete structured projection that can be selected when its primary result exceeds a bounded viewport. This applies to both diagrammatic and already-structured families. |

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
| Flowchart / graph | Partial | Diagrammatic | Yes | Root directions, Dagre-compatible ranking, explicit pinned-shape dispositions, common diagrammatic node shapes, terminal-cell wrapped node labels, independent endpoint markers, normal/dotted/thick/invisible edge semantics, labels, subgraphs, nested groups, first-parent compound ownership, and scene-level route occupancy. | Icons, images, callbacks, links, some uncommon shapes, arbitrary dense-route candidate policy, and mixed-stroke crossing ownership remain unsupported or incomplete; bounded fallback is a typed compatibility projection, not clipped geometry. |
| Sequence | Partial | Diagrammatic | Yes | Mermaid-valid spaced/Unicode participant IDs, typed headless/filled/cross/point/bidirectional/half-arrow messages, central decorations, notes, lifecycles, boxes, and participant-bounded nested control frames. | Actor presentation metadata and links are accepted but omitted from the primary scene; bounded fallback discloses typed fields as structured text. |
| State | Partial | Diagrammatic | Yes | States, transitions, notes, graph-like pseudostates, groups, and terminal colors. | Some presentation metadata and future shape variants are approximated; bounded fallback is a typed compatibility projection. |
| Class | Partial | Diagrammatic | Yes | Class structure, notes, namespaces, four directions, independent source/target relation markers, shared relation components, a strict planar K2×2 four-node/four-edge cycle with four disjoint routes, simple sibling-namespace / namespace-to-root / nested-sibling facade routing with length-framed leaf identity, and explicit relation summaries. | The strict K2×2 layout is a bounded topology-specific path, not support for arbitrary bounded or dense graphs. Dense or colliding namespace-crossing scenes, port-incompatible lanes, and other collision-prone relationships can use lossless `relations:` output. |
| ER | Partial | Diagrammatic | Yes | Entities, attributes, key tokens, attribute comments, four directions, relationship labels/cardinalities including the parent diamond, shared relation components, a strict planar K2×2 four-node/four-edge cycle with four disjoint routes, and explicit relation summaries. | The strict K2×2 layout is a bounded topology-specific path, not support for arbitrary bounded or dense graphs. Port-incompatible and dense/collision-prone topology can use lossless `relations:` output; accessibility, Mermaid diagram source comments, and styling metadata are intentionally omitted. |
| XYChart | Partial | Diagrammatic | Yes | Model-owned x/y samples and point labels, band/linear axes, negative/reversed/degenerate ranges, grouped bars, connected topology-resolved lines, mixed series, titles, legends, display policy, injective length-framed disclosure, empty-chart metadata reports, and horizontal/vertical variants. Parser-produced x coordinates derive from the typed axis/category domain and sample order. | Browser hover is replaced by terminal disclosure; terminal coordinates are quantized, cross-series same-cell ownership remains approximate, unknown direct-model orientations and band y-axes are rejected, and accessibility title/description metadata is intentionally omitted. |

## Structured-Text Outputs

These families intentionally produce a terminal-safe semantic report or outline rather than a
diagram. The output is useful for logs, debugging, and narrow terminals, and can aid accessibility
when the typed model retains the relevant metadata, but it is
excluded from diagrammatic-family counts and from claims that Merman has an ASCII geometry
implementation. It is selected only by the explicit viewport `Fallback` policy; resource limits and
cancellation remain hard errors and never trigger this projection.

| Mermaid family | Semantic coverage | Primary projection | What the report preserves | Why it is not counted as an ASCII diagram |
| --- | --- | --- | --- | --- |
| Gantt | Partial | Structured text | Titles, empty sections, task ids, resolved and adjusted dates, time-of-day precision, start/end constraints such as multi-id `after`/`until`, tags, and relevant scheduling options. Authored strings and lists use quoted byte-length framing so field delimiters remain recoverable. | No timeline geometry; links and click callbacks are metadata-only; duplicate or empty task ids are rejected. |
| GitGraph | Partial | Structured text | Commits, branches, parent topology, merges, tags, cherry-picks, ordering, and semantic type/id override disclosure. | Does not draw a full Git lane graph; unknown direct-model commit types are rejected. |
| Journey | Partial | Structured text | Sections, tasks, actors, and scores, with quoted byte-length framing for authored strings and list items. | Does not draw Mermaid journey geometry. |
| Kanban | Partial | Structured text | Columns, stable group/card ids, group parent ownership, assignments, metadata, and deterministic `Unassigned` grouping. Authored labels and metadata use quoted byte-length framing. | Drag/drop and nested board geometry are not represented; group parent ownership is disclosed as text; duplicate or empty ids are rejected. |
| Mindmap | Partial | Structured text | Hierarchical nodes, stable authored ids, labels, cycles/disconnected components, charset-aware connectors, and shape/icon/section disclosure as an outline. Authored fields use quoted byte-length framing. | Rich browser node geometry is not reproduced; duplicate internal/authored ids, missing authored ids, duplicate edges, and missing endpoints are rejected. |
| Packet | Partial | Structured text | Bit ranges and labels in ordered terminal rows. | Output does not preserve spatial bit widths; browser-oriented styling is not represented. |
| Timeline | Partial | Structured text | Direction, sections, and events in ordered grouped text; authored task, event, section, title, and accessibility fields use quoted byte-length framing. | Does not draw Mermaid timeline geometry; parser bookkeeping score is omitted. |
| TreeView | Partial | Structured text | Root and node identities, authored levels, hierarchy, file/directory types, charset-aware connectors, and icon/class/description disclosure. Authored fields use quoted byte-length framing. | It is an outline rather than two-dimensional diagram geometry; duplicate node ids and unknown node types are rejected, and browser icons/CSS classes are disclosed rather than styled. |

## Unsupported Families

These families may parse or render to SVG elsewhere in `merman`, but they are not currently exposed
as ASCII/Unicode render targets. Railroad, Requirement, Ishikawa, and Quadrant have completed the
R34 evaluation rather than merely waiting in an unclassified backlog; the tracked
[Phase 0-3 and admission report](ASCII_PHASE_GATE_REPORT.md) records their representative tasks,
fixtures, spatial facts, StructuredText comparison, and 80/100/120-column rejection matrices.

| Mermaid family | ASCII status | Notes |
| --- | --- | --- |
| Architecture | Unsupported | Rich grouped architecture geometry is SVG-focused. |
| Block | Unsupported | Block layout is SVG-focused. |
| C4 | Unsupported | C4 views remain SVG/headless-render output. |
| Cynefin | Unsupported | No terminal projection has been admitted. |
| Event Modeling | Unsupported | No terminal projection has been admitted. |
| Info | Unsupported | Not useful as terminal diagram output today. |
| Ishikawa | Unsupported | [R34 evaluation rejected the current depth-limited prototype](ASCII_PHASE_GATE_REPORT.md#ishikawa-r34); its typical case adds spatial value, but dense recursive ownership is incomplete. |
| Pie | Unsupported | No terminal pie/chart approximation yet. |
| Quadrant | Unsupported | [R34 evaluation rejected the current collision prototype](ASCII_PHASE_GATE_REPORT.md#quadrant-r34); small/typical cases add spatial value, but dense relative order collapses at all required widths. |
| Radar | Unsupported | No terminal radar approximation yet. |
| Railroad | Unsupported | [R34 evaluation rejected the current route-expansion prototype](ASCII_PHASE_GATE_REPORT.md#railroad-r34); choice paths are visible, but repetition loops and dense shared prefixes are not. |
| Requirement | Unsupported | [R34 evaluation rejected the current tree-expansion prototype](ASCII_PHASE_GATE_REPORT.md#requirement-r34); single relations are exact, but dense convergences lose shared endpoint ownership. |
| Sankey | Unsupported | Flow widths are SVG-specific. |
| Treemap | Unsupported | Rectangle packing is SVG-specific. |
| Venn | Unsupported | No terminal set-overlap projection has been admitted. |
| Wardley | Unsupported | No terminal map projection has been admitted. |
| ZenUML | Unsupported | The family has a dedicated typed semantic model and SVG renderer, but no family-owned terminal projection has been admitted. It must not be translated through Sequence as an ASCII shortcut. |

## Playground Filtering

The playground "ASCII supported" filter uses the runtime capability metadata when WASM is ready and
a tracked family/projection fallback copy before WASM finishes loading. The filter is currently
family-level:
every example whose diagram type has an available terminal projection is included, so membership is
not a separate claim that every semantic detail in that example is preserved. Basic `classDiagram`
output is supported, including nested namespace containers. The preview and export UI show
diagrammatic coverage or the Structured Text projection plus a concise limit for the active diagram
type. Simple sibling-namespace, namespace-to-root, and nested-sibling relationships route through
the nearest namespace facades with length-framed leaf identity; dense or colliding namespace scenes
remain partial and use the lossless `relations:` fallback. Class and ER strict planar K2×2
four-node/four-edge components use their bounded four-route diagrammatic layout; this does not
promote arbitrary bounded or dense topology out of the same lossless fallback boundary.

## Testing Policy

- Use exact snapshots when the ASCII shape itself is the contract.
- Use semantic assertions for structured-text output and relation-summary fallback, ensuring every retained entity/endpoint/label remains visible.
- Use explicit `UnsupportedFeature` tests for unsupported semantics instead of silently dropping
  Mermaid input.
- Keep Class/ER dense topology cases on the shared `relation_graph` summary path when routed output
  would overlap boxes; a resource-limit error remains an error and never becomes a summary.
- Keep the strict planar K2×2 exception covered by the Class/ER parser tests plus the shared
  `relation_graph` tests for declaration-order stability, four disjoint routes, exact/N−1 work
  admission, and speculative-work/document-cell ownership on collision fallback.
