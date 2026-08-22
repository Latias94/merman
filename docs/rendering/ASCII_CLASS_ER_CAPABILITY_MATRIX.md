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
assert the topology policy directly instead of inferring it only from rendered text. One targeted
topology exception is admitted: a strict planar K2×2 component with four nodes and four unique
relations uses a bounded cycle layout with four disjoint routes. That exception is stable under
relation declaration reordering and does not claim arbitrary bounded, crossing, or dense topology.

## Class Diagram Matrix

| Surface | Reference evidence | `merman` status | Fixture strategy |
| --- | --- | --- | --- |
| Class boxes, members, methods, annotations, including CJK/emoji member text | `beautiful-mermaid` parser/integration tests plus local wide-text coverage | Supported | Parser-backed semantic tests and local semantic fixtures |
| Directional association / dependency / inheritance / realization / aggregation / composition | `beautiful-mermaid` class arrow tests | Supported | Routed-grid fixtures and exact snapshots |
| Plain association (`--`, `..`) | `beautiful-mermaid` class parser and ASCII tests | Supported | Routed-grid and dense-summary regressions |
| Relationship labels and multiline labels, including CJK/emoji summary labels | `beautiful-mermaid` integration tests plus local wide-text coverage | Supported | Routed-grid, structured-summary, and local semantic fixtures |
| Same-endpoint lanes, reverse lanes, bounded cycles, spanning routes | `beautiful-mermaid` ASCII tests | Supported | Routed-grid fixtures |
| Strict planar K2×2: four nodes and four unique relations forming one four-edge cycle | Local topology policy | Supported through the bounded planar cycle layout; declaration order is stable and all four routes are disjoint | `class_parser_k2_2_relationships_use_a_bounded_planar_layout`, `strict_k2_2_uses_a_direction_independent_cycle_with_four_disjoint_routes`, and `strict_k2_2_geometry_is_stable_when_declarations_are_reordered` |
| Other dense, crossing, or collision-prone layouts | `beautiful-mermaid` ASCII tests plus local fallback policy | Supported through lossless `relations:` output, not arbitrary diagrammatic routing | Structured summary fixtures; `strict_k2_2_route_batch_admits_exact_work_and_rolls_back_n_minus_one` and `strict_k2_2_overlay_collision_keeps_speculative_work_but_discards_document_cells` cover the admitted path's resource/fallback boundary |
| Tight `max_ascii_grid_cells` budgets | Local policy | Supported hard error | Structured resource-error fixture with exact details |
| Disconnected components / isolated nodes | `beautiful-mermaid` disconnected-layout patterns plus local component-separation coverage | Supported | Local semantic fixtures with component-separation assertions |
| Namespace-qualified class names | Local semantic tests | Supported | Local semantic fixtures |
| Simple sibling-namespace, namespace-to-root, and nested-sibling relationships | Mermaid 11.16.1 typed namespace ownership plus local semantic tests | Supported through nearest-scope namespace facades with byte-length-framed leaf identity | Parser-backed namespace routing tests; dense/colliding scenes retain lossless summary fallback |
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
| Strict planar K2×2: four entities and four unique relationships forming one four-edge cycle | Local topology policy | Supported through the bounded planar cycle layout; declaration order is stable and all four routes are disjoint | `er_parser_k2_2_relationships_use_a_bounded_planar_layout`, `strict_k2_2_uses_a_direction_independent_cycle_with_four_disjoint_routes`, and `strict_k2_2_geometry_is_stable_when_declarations_are_reordered` |
| Other dense, crossing, or collision-prone layouts | `beautiful-mermaid` ER ASCII tests plus local fallback policy | Supported through lossless `relations:` output, not arbitrary diagrammatic routing | Structured summary fixtures; `strict_k2_2_route_batch_admits_exact_work_and_rolls_back_n_minus_one` and `strict_k2_2_overlay_collision_keeps_speculative_work_but_discards_document_cells` cover the admitted path's resource/fallback boundary |
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

- Strict planar K2×2 is a deliberately narrow admitted topology. Arbitrary bounded or dense
  Class/ER relation graphs remain outside that diagrammatic claim and continue to use the lossless
  `relations:` fallback when crossing, port-fit, route, or overlay policy rejects the routed scene.
- New SVG-only affordances should still be treated as new capabilities, not inferred from the
  current ASCII contract.
