# ASCII Reference Comparison

Status: living comparison note
Last updated: 2026-06-26

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
- Mindmap, TreeView, Timeline, Gantt, Journey, Kanban, Packet, and GitGraph now render as compact
  terminal summaries; keep those projections readable and headless rather than browser-shaped.
- CJK/emoji terminal-cell coverage is semantic, not byte-exact: primitive cell ownership plus
  Flowchart, Sequence, Class, ER, and XYChart tests prove visibility/alignment without copying
  reference spacing.
- Class and ER relation output now deliberately shares the `relation_graph` seam. Adapters preserve
  Mermaid-family semantics, while shared relation planning owns bounded iterative layer-order sweeps,
  lane placement, routed grids, and structured dense/grid-budget/collision summary fallback.
  Independent relation subgraphs split before planning, so unrelated pairs do not share one grid
  budget. Summary reasons are preserved at that seam for direct topology-policy tests.
- The SVG compare CLI keeps per-diagram entrypoints, but common fixture discovery, upstream/local
  SVG loading, DOM checks, local SVG writing, and result sections belong to the shared `xtask`
  compare harness. Adapter code should supply render-specific policy, not reimplement the harness.

## Reference Corpus Snapshot

`repo-ref/mermaid-ascii/cmd/testdata` currently provides the copied exact-output oracle admitted
by this crate: 54 `ascii` graph fixtures, 25 `extended-chars` graph fixtures, 12 `sequence`
fixtures, and 5 `sequence-ascii` fixtures. Its 3 `multibyte` examples for accented Latin, Greek,
and Cyrillic labels are useful semantic evidence, but they are not admitted as byte-level fixtures:
`merman-ascii` preserves the labels readably while intentionally not copying the upstream LR label
spacing byte-for-byte.

`repo-ref/beautiful-mermaid/src/__tests__/testdata` currently has 63 `ascii` fixtures and 37
`unicode` fixtures, plus focused tests for edge styles, multiline labels, class arrows, and
`xychart-beta` ASCII rendering. Treat that corpus as capability discovery. Promote an idea from it
by writing a small local semantic fixture or model test, not by treating its output as an official
Mermaid terminal standard.

Promoted local probes currently cover `beautiful-mermaid`-informed ampersand flowchart fan-in and
fan-out, Class annotations plus methods, ER attributes plus identifying relationships, Sequence
multi-message ordering, and XYChart multi-series value disclosure. These live under
`crates/merman-ascii/tests/testdata/local-semantic/` or focused renderer tests and assert local
Mermaid-visible behavior rather than copied spacing.

Runtime ASCII capability metadata mirrors this policy in
`crates/merman-ascii/src/capability.rs`: each comparison source is tagged as a copied
`mermaid-ascii` oracle, `beautiful-mermaid` prior art, a local semantic probe, a local advantage, or
support/gap documentation. `beautiful-mermaid` paths in that metadata are evidence references only;
they do not authorize byte-for-byte expected output.

## Fixture Admissibility

- Use copied `mermaid-ascii` fixtures when the family is graph or sequence and the upstream corpus
  is a good exact-output oracle. These are byte-level parity tests.
- Keep `mermaid-ascii` multibyte examples semantic unless the local routing policy deliberately
  changes to match their spacing. Label visibility, rectangular character grids, and no leaked
  markup are the relevant checks.
- Use `beautiful-mermaid` only as capability prior art. It can suggest coverage and layout ideas,
  but it is not a byte-level standard.
- Use self-authored local fixtures when the diagram is dense, family-specific, or semantically
  clearer than a copied render. Those fixtures live under `tests/testdata/local-semantic/`, and they
  are intentionally outside the copied fixture inventory gate.
- Class/ER fixtures are local semantic fixtures by default: `mermaid-ascii` does not cover those
  families, and `beautiful-mermaid` is capability prior art rather than an official output oracle.
  Admit routed-grid Class/ER cases when the topology remains readable; admit structured
  relation-summary cases when dense crossings or grid budgets make the honest terminal view a
  summary.
- Prefer semantic assertions for local fixtures: visible labels, direction relationships, grouping,
  routing reachability, unsupported-feature diagnostics, and absence of leaked implementation ids.
  Use exact ASCII snapshots only when the shape itself is the behavior under review.
- Reject a reference fixture as an oracle when it depends on a known reference shortcut, a browser
  rendering artifact, or terminal choices that are not implied by Mermaid semantics.

## Family Comparison

| Family | `mermaid-ascii` | `beautiful-mermaid` | `merman-ascii` | Readout |
| --- | --- | --- | --- | --- |
| Flowchart / graph | Exact copied-fixture parity for the narrow graph corpus, with LR/TD/TB routing and a small parser surface. | Broader graph ASCII ideas, ampersand fan-in/fan-out examples, richer shape handling, disconnected subgraph non-overlap checks, and more styling hooks, but `RL` is approximated as `LR`. | Flowchart is the strongest terminal family here: true `BT`/`RL`, boundary-aware subgraph routing, planner-owned vertical boundary label lanes, disconnected subgraph semantic coverage, ampersand fan-in/fan-out semantic probes, multiline edge labels, color roles, and a wider supported subset. | Keep `mermaid-ascii` for routing evidence and `beautiful-mermaid` for UI ideas, but keep Mermaid semantics first. |
| Sequence | Exact copied-fixture parity for a compact sequence corpus. | Much broader parser/layout coverage, including notes, blocks, theming, and ASCII/Unicode variants. | Typed sequence support is already beyond the narrow reference: activations, create/destroy, boxes with inner padding, control blocks, mirror actors, and color roles all exist. | Remaining work is mostly layout polish and boundary tightening, not parser rescue. |
| Class | Not part of the reference scope. | Full class parser/layout/ASCII, with compartments, annotations, multiline labels, and arrow-direction handling. | Supported subset through the shared `relation_graph` seam, with annotation and method semantic probes, multiline relationship labels, self-relation loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, spanning routes, cyclic reverse-span lanes, structured dense/grid-budget relation-summary fallback, dense multiline local semantic fixtures, and typed role colors. | Extend from typed relation facts, not from parser shape. |
| ER | Not part of the reference scope. | Full ER parser/layout/ASCII, including crow's foot notation, multiline relationship labels, and attribute sections. | Supported subset through the same `relation_graph` seam, with entity boxes, attributes plus key markers, cardinality markers, multiline relationship labels, self-relationship loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, cyclic reverse-span lanes, structured dense/grid-budget relation-summary fallback, and dense multiline local semantic fixtures. | Relation layout is the shared seam; cardinality and relationship identity stay family-specific. |
| State | Not part of the reference scope. | State diagram support rides the broader ASCII pipeline and gives useful layout ideas. | Supported subset with start/end, fork/join/choice, notes, composite states, divider regions, and role colors. | Keep state honest to the typed model; do not try to copy browser shapes literally. |
| XYChart | Not part of the reference scope. | Full xychart ASCII/SVG family, including legends, tooltips, and CSS-variable-driven palette behavior. | Compact terminal plots with bars, lines, mixed charts, horizontal mode, configurable compact plot areas, multi-series legend rows that use typed plot titles when present, axis visibility controls, Mermaid data-label display policy, and terminal `values:` disclosure rows for line and multi-series charts. | Plot planning is split from row rendering; future work should focus on richer dense-layout policy rather than copying browser hover behavior. |
| Mindmap / TreeView | Not part of the reference scope. | Broader mindmap/tree examples can suggest readable outline shapes. | Compact hierarchy summaries with preserved order and wrapped labels. | Keep the output readable and compact; do not imitate browser geometry. |
| Timeline / Gantt | Not part of the reference scope. | Broader schedule renderers can suggest readable summary patterns. | Readable rows that preserve sections, tasks, spans, and flags. | Favor honest text summaries over pseudo-graphs. |
| Journey / Kanban | Not part of the reference scope. | Broader board renderers can suggest grouping and actor/card metadata patterns. | Readable summaries that preserve section order, actor order, and card metadata. | Keep the projection stable and compact. |
| Packet / GitGraph | Not part of the reference scope. | Broader process-diagram examples can suggest readable lane summaries. | Readable summaries that preserve ranges, parents, tags, commit order, and warnings. | Favor traceable text over decorative pseudo-graphs. |

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

- CJK and emoji placement is covered for the current renderer families; keep the same semantic
  gate for new families and more complex grapheme clusters.
- Flowchart route-label placement beyond the shipped boundary transit-lane policy: general
  grid-path and dense multi-edge labels still need explicit route-plan policy before complex local
  fixtures should be admitted.
- Class and ER dense relation topologies beyond the current fallback; new policy decisions should
  keep explicit `LayeredRelationSummaryReason` variants for route, overlay, crossing, and grid
  budget boundaries.
- XYChart dense-layout policy beyond the shipped compact plot and `values:` disclosure rows.
