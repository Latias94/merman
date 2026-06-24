# ASCII Class / ER Capability Matrix

This document compares the current `merman` ASCII renderer against the two local reference
repositories:

- `repo-ref/mermaid-ascii`
- `repo-ref/beautiful-mermaid`

Use `mermaid-ascii` as a historical baseline for graph/sequence output only. Use
`beautiful-mermaid` as capability evidence for Class and ER semantics, not as a byte-level oracle.

## Class Diagram Matrix

| Surface | Reference evidence | `merman` status | Fixture strategy |
| --- | --- | --- | --- |
| Class boxes, members, methods, annotations | `beautiful-mermaid` parser/integration tests | Supported | Parser-backed semantic tests |
| Directional association / dependency / inheritance / realization / aggregation / composition | `beautiful-mermaid` class arrow tests | Supported | Routed-grid fixtures and exact snapshots |
| Plain association (`--`, `..`) | `beautiful-mermaid` class parser and ASCII tests | Supported | Routed-grid and dense-summary regressions |
| Relationship labels and multiline labels | `beautiful-mermaid` integration tests | Supported | Routed-grid fixtures |
| Same-endpoint lanes, reverse lanes, cycles, crossings, spanning routes | `beautiful-mermaid` ASCII tests | Supported | Routed-grid fixtures |
| Dense layouts that should collapse to relation summary | `beautiful-mermaid` ASCII tests | Supported | Structured summary fixtures |
| Tight `max_grid_cells` budgets | Local policy | Supported | Structured summary fixture with explicit budget |
| Namespace-qualified class names | Local semantic tests | Supported | Local semantic fixtures |
| Endpoint labels / cardinality strings attached to a relation | Mermaid class cardinality tests and `beautiful-mermaid` parser/renderer | Supported | Exact vertical fixtures and summary regressions |
| Lollipop relations | Not represented in ASCII renderer | Explicit unsupported | Keep as `UnsupportedFeature` model tests |
| Multiple markers on one relation | Not represented in ASCII renderer | Explicit unsupported | Keep as `UnsupportedFeature` model tests |

## ER Diagram Matrix

| Surface | Reference evidence | `merman` status | Fixture strategy |
| --- | --- | --- | --- |
| Entity boxes, attributes, PK / UK / FK tokens, comments | `beautiful-mermaid` ER parser/integration tests | Supported | Parser-backed semantic tests |
| Identifying and non-identifying relationships | `beautiful-mermaid` ER parser/integration tests | Supported | Routed-grid fixtures |
| Cardinality variants (`||`, `o|`, `|{`, `o{`, and reversed forms) | `beautiful-mermaid` ER parser tests | Supported | Routed-grid fixtures |
| Relationship labels and multiline labels | `beautiful-mermaid` ER integration tests | Supported | Routed-grid fixtures |
| Same-endpoint lanes, reverse lanes, cycles, crossings, spanning routes | `beautiful-mermaid` ER ASCII tests | Supported | Routed-grid fixtures |
| Dense layouts that should collapse to relation summary | `beautiful-mermaid` ER ASCII tests | Supported | Structured summary fixtures |
| Tight `max_grid_cells` budgets | Local policy | Supported | Structured summary fixture with explicit budget |
| Unknown cardinality markers | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |
| Unknown relationship identification types | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |
| Missing endpoint entities | Not represented in reference ASCII output | Explicit unsupported | Keep as `UnsupportedFeature` model tests |

## Fixture Guidance

- Use small local semantic fixtures when the input itself is the behavior under review.
- Prefer exact snapshots only when the text shape is the behavior.
- Prefer parser-backed semantic assertions for unsupported boundaries.
- Do not treat `beautiful-mermaid` as a canonical golden corpus for Class or ER output.

## Current Gaps Worth Watching

- None for the baseline Class / ER comparison tracked in this document. New SVG-only affordances
  should still be treated as new capabilities, not inferred from the current ASCII contract.
