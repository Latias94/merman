# ASCII Reference Comparison

Status: living comparison note
Last updated: 2026-06-24

This note compares `merman-ascii` with the two local reference checkouts:

- `repo-ref/mermaid-ascii`
- `repo-ref/beautiful-mermaid`

It is not a support gate. The shipped support matrices and gap registry remain the authoritative
product boundary.

## Short Read

- `mermaid-ascii` is the narrow reference. It is strongest as a copied-fixture oracle for graph and
  sequence text output.
- `beautiful-mermaid` is the broad reference. It covers more families and more terminal styling
  ideas, but it is not always semantically faithful to Mermaid.
- `merman-ascii` is the product renderer. It is typed-model driven, keeps the model boundary
  explicit, and should prefer honest terminal semantics over browser-shaped approximations.

## Fixture Admissibility

- Use copied `mermaid-ascii` fixtures when the family is graph or sequence and the upstream corpus
  is a good exact-output oracle.
- Use `beautiful-mermaid` only as capability prior art. It can suggest coverage and layout ideas,
  but it is not a byte-level standard.
- Use self-authored local fixtures when the diagram is dense, family-specific, or semantically
  clearer than a copied render. Those fixtures live beside the tests that use them, such as
  `tests/testdata/local-semantic/`, and they are intentionally outside the copied fixture inventory
  gate.

## Family Comparison

| Family | `mermaid-ascii` | `beautiful-mermaid` | `merman-ascii` | Readout |
| --- | --- | --- | --- | --- |
| Flowchart / graph | Exact copied-fixture parity for the narrow graph corpus, with LR/TD/TB routing and a small parser surface. | Broader graph ASCII ideas, richer shape handling, and more styling hooks, but `RL` is approximated as `LR`. | Flowchart is the strongest terminal family here: true `BT`/`RL`, boundary-aware subgraph routing, color roles, and a wider supported subset. | Keep `mermaid-ascii` for routing evidence and `beautiful-mermaid` for UI ideas, but keep Mermaid semantics first. |
| Sequence | Exact copied-fixture parity for a compact sequence corpus. | Much broader parser/layout coverage, including notes, blocks, theming, and ASCII/Unicode variants. | Typed sequence support is already beyond the narrow reference: activations, create/destroy, boxes, control blocks, mirror actors, and color roles all exist. | Remaining work is mostly layout polish and boundary tightening, not parser rescue. |
| Class | Not part of the reference scope. | Full class parser/layout/ASCII, with compartments, annotations, and arrow-direction handling. | Supported subset via the layered relation planner, with same-endpoint lanes, spanning routes, dense/cyclic fallback, and typed role colors. | Extend from typed relation facts, not from parser shape. |
| ER | Not part of the reference scope. | Full ER parser/layout/ASCII, including crow's foot notation and attribute sections. | Supported subset via the same layered relation machinery, with entity boxes, cardinality markers, and dense/cyclic fallback. | Relation layout is the seam worth deepening. |
| State | Not part of the reference scope. | State diagram support rides the broader ASCII pipeline and gives useful layout ideas. | Supported subset with start/end, fork/join/choice, notes, composite states, divider regions, and role colors. | Keep state honest to the typed model; do not try to copy browser shapes literally. |
| XYChart | Not part of the reference scope. | Full xychart ASCII/SVG family, including legends, tooltips, and CSS-variable-driven palette behavior. | Compact terminal plots with bars, lines, mixed charts, and horizontal mode. | Legends and larger plot-area planning are the next obvious deepening slice. |

## Intentional Differences

- True `RL` inversion is intentional. Treating `RL` as `LR` is a reference-implementation shortcut, not
  a product goal.
- Cyclic class and ER shapes should keep rendering through the layered planner when the typed model
  can support a readable fallback.
- Wide-cell handling must treat terminal continuation cells as shared ownership, not as independent
  characters.
- Sequence `rect` and box colors should stay bounded by what the typed model and terminal can render
  without inventing browser-only semantics.

## Remaining Pressure

- CJK and emoji placement across renderers.
- Flowchart route families beyond the current layered planner.
- Class and ER dense relation topologies beyond the current fallback.
- XYChart legends and broader plot-area scaling.
