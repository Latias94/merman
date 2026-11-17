# ASCII Class / ER Capability Matrix

This document compares the current `merman` ASCII renderer against the two local reference
repositories:

- `repo-ref/mermaid-ascii`
- `repo-ref/beautiful-mermaid`

Use `mermaid-ascii` as a historical baseline for graph/sequence output only. Use
`beautiful-mermaid` as capability evidence for Class and ER semantics, not as a byte-level oracle.

## Architecture Note

Class and ER ASCII rendering share the `relation_graph` seam for relation routing, lane placement,
layered drawing, and structured summary fallback. The family adapters should keep only
Mermaid-specific semantics at their edges: Class markers, notes, lollipop/interface handling,
endpoint labels, ER cardinality, relationship identification, entity labels, and explicit
unsupported diagnostics. Dense crossings and route/overlay collisions may use the shared
`relations:` summary rather than renderer-local fallback branches. Resource limits are never
readability fallbacks: exceeding `max_ascii_grid_cells` or another ASCII limit returns a structured
resource error. Summary fallback reasons are preserved at the `relation_graph` seam so tests can
assert the topology policy directly instead of inferring it only from rendered text. Iterative
strongly connected components and the condensation DAG determine directed ranks. Bounded private
channels keep box contacts, exterior returns, and label reading areas distinct. Rings and DAG
diamonds use these shared rules in all four directions, replacing the earlier strict K2×2
exception. After route validation, shared axis compaction removes redundant straight-channel
rows and columns. It preserves box and text footprints, distinct terminals and bends, and at
least one cell between separated features; the compacted scene is validated again before paint.
This improves canonical geometry without admitting a separate Compact layout profile or changing
the authored direction. Dense topology can still select a complete summary.

Private-channel endpoints keep markers adjacent to their frames and place cardinality text beside
continuous vertical stems. Multiline endpoint text uses one left edge; measurement, collision
checks, compaction, and painting share that anchor. Endpoint text cannot replace its own stem or
another object's cells. Class route profiles retain each endpoint's measured text extent so port
selection excludes contacts whose labels already overlap a frame or occupied area. Unsafe
placement retains the complete `relations:` summary. Parser-backed Class controls cover all four
directions, both structural charsets, independent markers, headless
ends, and literal cardinality ownership; shared port tests retain the ER marker widths.

Viewport overflow has a separate Plain reading document: keyed declarations retain full names,
namespace/group ownership, members/attributes, interfaces/notes, and individual relationships.
Class marker types and endpoint labels remain independent. Qualified aliases retain their authored
reference and resolved namespace owner; interface references follow the primary topology's identity
selection, including repeated IDs. ER cardinalities attach to the actual
first/second endpoint and use conventional ranges, with a stroke legend for relationship
identification. Known defaults, redundant display labels, native generated ER IDs, and Class's
derived topology cache are omitted; nondefault metadata and source configuration remain complete.
The bounded serializer and reading-record writer are shared with other semantic fallbacks.

## Class Diagram Matrix

| Surface | Reference evidence | `merman` status | Fixture strategy |
| --- | --- | --- | --- |
| Class boxes, members, methods, annotations, including CJK/emoji member text | `beautiful-mermaid` parser/integration tests plus local wide-text coverage | Supported | Parser-backed semantic tests and local semantic fixtures |
| Directional association / dependency / inheritance / realization / aggregation / composition | `beautiful-mermaid` class arrow tests | Supported | Routed-grid fixtures and exact snapshots |
| Plain association (`--`, `..`) | `beautiful-mermaid` class parser and ASCII tests | Supported | Routed-grid and dense-summary regressions |
| Relationship labels and multiline labels, including CJK/emoji summary labels | `beautiful-mermaid` integration tests plus local wide-text coverage | Supported | Routed-grid, structured-summary, and local semantic fixtures |
| Same-endpoint lanes, reverse lanes, bounded cycles, spanning routes | `beautiful-mermaid` ASCII tests | Supported | Routed-grid fixtures |
| Directed rings and DAG diamonds | Local topology policy | Shared directed ranks and private channels; declaration order is stable and labels keep reading clearance | `class_directed_cycles_share_topology_across_directions_and_declaration_order`, `class_diamond_uses_directed_ranks_in_every_direction`, and shared topology/route tests |
| Other dense, crossing, or collision-prone layouts | `beautiful-mermaid` ASCII tests plus local fallback policy | Lossless `relations:` output remains available when bounded routing is unsafe | Structured summary fixtures and shared exact/N−1 resource/fallback tests |
| Tight `max_ascii_grid_cells` budgets | Local policy | Supported hard error | Structured resource-error fixture with exact details |
| Disconnected components / isolated nodes | `beautiful-mermaid` disconnected-layout patterns plus local component-separation coverage | Supported | Local semantic fixtures with component-separation assertions |
| Namespace-qualified class names | Local semantic tests | Supported | Local semantic fixtures |
| Simple sibling-namespace, namespace-to-root, and nested-sibling relationships | Pinned Mermaid typed namespace ownership plus local semantic tests | Supported through nearest-scope namespace facades with byte-length-framed leaf identity | Parser-backed namespace routing tests; dense/colliding scenes retain lossless summary fallback |
| Endpoint labels / cardinality strings attached to a relation | Mermaid class cardinality tests and `beautiful-mermaid` parser/renderer | Supported | Exact vertical fixtures and summary regressions |
| Notes and note-for links | Mermaid class parser / SVG behavior | Supported | Local semantic fixtures with exact snapshots |
| Lollipop relations and interface nodes | Mermaid class parser / SVG behavior | Supported | Routed-grid fixtures and summary regressions |
| Multiple markers on one relation | Not represented in ASCII renderer | Explicit unsupported | Keep as `UnsupportedFeature` model tests |

## ER Diagram Matrix

| Surface | Reference evidence | `merman` status | Fixture strategy |
| --- | --- | --- | --- |
| Entity boxes, attributes, PK / UK / FK tokens, comments, including CJK/emoji attributes | `beautiful-mermaid` ER parser/integration tests plus local wide-text coverage | Supported | Parser-backed semantic tests and local semantic fixtures |
| Identifying and non-identifying relationships | `beautiful-mermaid` ER parser/integration tests | Supported | Routed-grid fixtures |
| Cardinality variants (`||`, `o|`, `|{`, `o{`, and reversed forms) | `beautiful-mermaid` ER parser tests | Supported | Routed-grid fixtures |
| Relationship labels and multiline labels, including CJK/emoji summary labels | `beautiful-mermaid` ER integration tests plus local wide-text coverage | Supported | Routed-grid, structured-summary, and local semantic fixtures |
| Same-endpoint lanes, reverse lanes, bounded cycles, spanning routes | `beautiful-mermaid` ER ASCII tests | Supported | Routed-grid fixtures |
| Directed rings and DAG diamonds | Local topology policy | Shared directed ranks and private channels; independent cardinalities and labels remain family-owned | `er_directed_cycles_share_topology_across_directions_and_declaration_order`, `er_diamond_uses_directed_ranks_and_private_relationship_labels`, and shared topology/route tests |
| ER groups, nested membership, and group endpoints | Pinned Mermaid ER database and parser semantics; moving `mermaid-ascii` rejects groups | ER-owned nested frames, including empty groups, preserve typed entity/group endpoints. Connections through an ancestor frame disclose the actual member and facade; ancestor/member edges use complete typed summaries beside the group geometry. | Group endpoint, facade, nesting, cardinality, viewport/report, cancellation, and resource tests in `er_model.rs`, `relation_reading_fallback.rs`, and `output_report.rs` |
| Other dense, crossing, or collision-prone layouts | `beautiful-mermaid` ER ASCII tests plus local fallback policy | Lossless `relations:` output remains available when bounded routing is unsafe | Structured summary fixtures and shared exact/N−1 resource/fallback tests |
| Tight `max_ascii_grid_cells` budgets | Local policy | Supported hard error | Structured resource-error fixture with exact details |
| Disconnected components / isolated entities | `beautiful-mermaid` disconnected-layout patterns plus local component-separation coverage | Supported | Local semantic fixtures with component-separation assertions |
| Unknown cardinality markers | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |
| Unknown relationship identification types | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |
| Missing endpoint entities | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |

## Fixture Guidance

- Use small local semantic fixtures when the input itself is the behavior under review.
- Prefer exact snapshots only when the text shape is the behavior.
- Prefer parser-backed semantic assertions for unsupported boundaries.
- Do not treat `beautiful-mermaid` as a canonical golden corpus for Class or ER output.

## Current Gaps Worth Watching

- Private-channel search is bounded and can miss a readable arrangement. Dense Class/ER scenes
  continue to use the lossless `relations:` fallback when contact, route, or overlay admission fails.
- ER group scopes without an authored direction use TB; the parent scope places the completed
  frame. Deep member connections use explicitly named group facades rather than direct contacts
  to the inner entity. Duplicate group IDs, multiple ownership, cyclic membership, ambiguous
  routing IDs, and relationships to hidden shadow entities remain precise diagnostics. Forward
  references are not silently retargeted to a later group. Reading records resolve memberships
  by authored names and relationships by stored route IDs.
- New SVG-only affordances should still be treated as new capabilities, not inferred from the
  current ASCII contract.
