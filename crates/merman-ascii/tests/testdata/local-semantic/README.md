# Local Semantic Fixtures

This directory stores self-authored ASCII fixtures that are meant to validate semantic behavior
rather than copied upstream output.

- Use it when the copied `mermaid-ascii` corpus is a poor oracle for the shape you want to test.
- Keep the files small and focused on the behavior under review.
- Assert semantic invariants in tests unless the output shape itself is the behavior under review.
- Do not add copied parity fixtures here; those belong under `tests/testdata/mermaid-ascii/`.

## Admission Rules

- Copied fixtures are allowed only when a reference corpus is a byte-level oracle. Today that means
  the admitted `mermaid-ascii` graph and sequence fixtures, not Class/ER.
- `beautiful-mermaid` is capability evidence for Class, ER, XYChart, color, and multiline terminal
  behavior. It can suggest a local fixture, but it is not an official golden-output standard.
- Local semantic fixtures should name the Mermaid behavior being protected: visible labels,
  preserved relationship direction, routed reachability, grouped content, supported terminal color
  roles, or explicit unsupported-feature diagnostics.
- Exact text snapshots are appropriate only when the text shape is itself the behavior. Otherwise,
  prefer targeted semantic assertions so future layout improvements do not rewrite unrelated
  fixtures.

## Class/ER Relation Fixtures

Class and ER relation fixtures are split by topology readability:

- Use routed-grid fixtures when the relation graph has a deterministic, readable terminal path:
  chains, stars, same-endpoint lanes, bidirectional same-pair lanes, simple spanning lanes, and
  cycle-closing lanes.
- Use structured relation-summary fixtures when a relation graph is too dense for a readable
  deterministic layered layout, or when the routed scene exceeds `AsciiRenderOptions::max_grid_cells`.
- Summary fixtures must keep every endpoint, connector, and label line visible; multiline Mermaid
  labels should become continuation rows rather than slash-joined text or leaked `<br>` markup.

Current examples:

- `class/dense_relations.mmd`
- `class/dense_multiline_relations.mmd`
- `er/dense_relations.mmd`
- `er/dense_multiline_relations.mmd`
- `flowchart/multi_boundary_routes.mmd`
- `flowchart/nested_direction_boundary.mmd`
- `flowchart/sibling_boundary_routes.mmd`
- `sequence/dense_control_rows.mmd`
- `state/composite_boundary.mmd`
- `xychart/mixed_small.mmd`
