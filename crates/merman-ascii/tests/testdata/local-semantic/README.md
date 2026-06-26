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
- Reference examples that are useful but not exact-output oracles, such as `mermaid-ascii`
  multibyte labels or `beautiful-mermaid` xychart/class/ER examples, should be re-expressed as
  small semantic tests. Preserve the Mermaid behavior under review, not the reference renderer's
  incidental spacing.
- Local semantic fixtures should name the Mermaid behavior being protected: visible labels,
  preserved relationship direction, routed reachability, grouped content, supported terminal color
  roles, or explicit unsupported-feature diagnostics.
- Exact text snapshots are appropriate only when the text shape is itself the behavior. Otherwise,
  prefer targeted semantic assertions so future layout improvements do not rewrite unrelated
  fixtures.
- CJK/emoji cases should assert terminal-cell semantics directly: labels stay visible, rows remain
  owned by the same canvas line, and Class/ER summaries keep every endpoint, connector, and wide
  label line visible. Do not copy reference-renderer spacing for multibyte examples.

## Class/ER Relation Fixtures

Class and ER relation fixtures are split by topology readability:

These fixtures protect the shared `relation_graph` seam. Class and ER adapters may differ in
markers, cardinality, labels, notes, and unsupported diagnostics, but routing, lane placement,
layered draw policy, and dense/grid-budget summary fallback should stay shared.

- Use routed-grid fixtures when the relation graph has a deterministic, readable terminal path:
  chains, stars, same-endpoint lanes, bidirectional same-pair lanes, simple spanning lanes, and
  cycle-closing lanes.
- Use structured relation-summary fixtures when a relation graph is too dense for a readable
  deterministic layered layout, or when the routed scene exceeds `AsciiRenderOptions::max_grid_cells`.
- Summary fixtures must keep every endpoint, connector, and label line visible; multiline Mermaid
  labels should become continuation rows rather than slash-joined text or leaked `<br>` markup.

| Fixture class | Use when | Assertions |
| --- | --- | --- |
| Routed grid | The topology has readable terminal routes. | Important endpoints, labels, markers, and compartments are visible; `relations:` is absent. |
| Structured summary | Dense crossings or grid budget make a routed grid misleading. | Every endpoint, connector, and label line is visible under `relations:`; `<br>` does not leak. |
| Unsupported boundary | Mermaid syntax has semantics the ASCII renderer cannot honestly represent yet. | Prefer focused parser/model tests that assert `UnsupportedFeature`; add a fixture only when the input itself documents a durable boundary. |

See [ASCII Class / ER Capability Matrix](../../../../../docs/rendering/ASCII_CLASS_ER_CAPABILITY_MATRIX.md) for the current comparison against `beautiful-mermaid` and `mermaid-ascii`.

Current covered Class capabilities include association (`--` / `..`), inheritance, realization, aggregation, composition, notes, lollipop/interface nodes, endpoint cardinality labels, CJK/emoji member and relationship labels, multiline labels, parallel lanes, crossing reroutes, disconnected components, dense summary fallback, and tight-budget summary fallback.
Current covered Class local semantic fixtures also include namespace-qualified relationships and wide member / summary-label coverage.
Current covered ER capabilities include entity attributes, CJK/emoji attribute and relationship labels, key/comment tokens, identifying and non-identifying relationships, normalized cardinality spellings (`}|` / `}o`), multiline labels, parallel lanes, crossing reroutes, disconnected components, dense summary fallback, and tight-budget summary fallback.
Current covered ER local semantic fixtures also include wide attribute / summary-label coverage.
Current explicit unsupported boundaries are covered by typed-model tests for Class multiple markers, plus ER unknown cardinality markers and unknown relationship identification types.

## Current Examples

Flowchart ASCII fixtures should protect planner-owned route semantics rather than copied spacing
from a reference renderer. `mermaid-ascii` and `beautiful-mermaid` are useful capability probes for
basic comparison, but complex grouped or cross-boundary diagrams should be judged by whether the
ASCII topology stays readable.

Use local semantic flowchart fixtures when the behavior depends on grouping, local `direction`
overrides, cross-boundary reachability, terminal-cell label width, or planner-owned label lanes.
Assertions should check visible endpoints, labels, relative ordering, and whether labels stay on
the intended semantic lane. Avoid exact whole-output snapshots unless the ASCII shape itself is
the behavior under review.

Current examples:

- `class/dense_relations.mmd`
- `class/dense_multiline_relations.mmd`
- `class/disconnected_components.mmd`
- `class/namespace_qualified_relationships.mmd`
- `class/note_for_service.mmd`
- `class/routed_relationship_variants.mmd`
- `class/standalone_note.mmd`
- `class/wide_members_and_summary_labels.mmd`
- `er/dense_relations.mmd`
- `er/dense_multiline_relations.mmd`
- `er/disconnected_components.mmd`
- `er/routed_schema_with_attributes.mmd`
- `er/wide_attributes_and_summary_labels.mmd`
- `flowchart/back_edge_labels.mmd`
- `flowchart/boundary_label_lane.mmd`
- `flowchart/cjk_boundary_routes.mmd`
- `flowchart/disconnected_subgraphs.mmd`
- `flowchart/multiline_edge_label.mmd`
- `flowchart/multi_boundary_routes.mmd`
- `flowchart/nested_direction_boundary.mmd`
- `flowchart/sibling_boundary_routes.mmd`
- `sequence/dense_control_rows.mmd`
- `sequence/self_messages_with_notes.mmd`
- `state/composite_boundary.mmd`
- `state/cjk_connection_lifecycle.mmd`
- `xychart/mixed_small.mmd`
- `xychart/horizontal_mixed_cjk.mmd`
