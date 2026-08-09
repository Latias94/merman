# ASCII Reference Comparison

Status: living comparison note
Last updated: 2026-08-09

This note records comparison evidence for the pinned upstream revisions used by `merman-ascii` and
notes what may be visible in the two optional local reference checkouts:

- `AlexanderGrooff/mermaid-ascii` at copied-source commit `6fffb8e2714acab2c4cb41c78894fabbc62cee56`; `repo-ref/mermaid-ascii` may move beyond it.
- `lukilabs/beautiful-mermaid` at reference commit `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`; `repo-ref/beautiful-mermaid` is research-only.

It is not a support gate. The shipped support matrices and gap registry remain the authoritative
product boundary.

## Short Read

- The pinned `mermaid-ascii` source is the narrow copied-fixture oracle for graph and sequence text
  output. The ignored local checkout inspected at `b1b35f67d6a5dd0699ccfc968c00a763db573076`
  also contains a newer ER renderer, but that post-snapshot code is discovery evidence only.
- `beautiful-mermaid` is the broad reference. It covers more families and more terminal styling
  ideas, but it is not always semantically faithful to Mermaid.
- `merman-ascii` is the product renderer. It is typed-model driven, keeps the model boundary
  explicit, and should prefer honest terminal semantics over browser-shaped approximations.
- Mindmap, TreeView, Timeline, Gantt, Journey, Kanban, Packet, and GitGraph now render as compact
  structured-text projections; keep those projections readable and headless rather than
  browser-shaped.
- The Phase 0-3 evidence and R34 admission decisions are tracked in
  `docs/rendering/ASCII_PHASE_GATE_REPORT.md`. Railroad, Requirement, Ishikawa, and Quadrant remain
  Unsupported because each current prototype fails at least one required dense or typed-topology
  case at 80/100/120 columns; Ishikawa typical and Quadrant small/typical still demonstrate useful
  spatial signals. This is a current-cycle admission result, not a claim of permanent terminal
  unsuitability.
- CJK/emoji terminal-cell coverage is semantic, not byte-exact: primitive cell ownership plus
  Flowchart, Sequence, Class, ER, and XYChart tests prove visibility/alignment without copying
  reference spacing.
- Class and ER relation output now deliberately shares the `relation_graph` seam. Adapters preserve
  Mermaid-family semantics, while shared relation planning owns bounded iterative layer-order sweeps,
  lane placement, routed grids, and structured crossing/port-fit/route/overlay-collision fallback.
  Independent relation subgraphs split before planning, so unrelated pairs do not force each other
  into one relation scene. Summary reasons are preserved at that seam for direct topology-policy tests.
- The SVG compare CLI keeps per-diagram entrypoints, but common fixture discovery, upstream/local
  SVG loading, DOM checks, local SVG writing, and result sections belong to the shared `xtask`
  compare harness. Adapter code should supply render-specific policy, not reimplement the harness.

## Reference Corpus Snapshot

The tracked corpus based on `mermaid-ascii` commit
`6fffb8e2714acab2c4cb41c78894fabbc62cee56`, plus the two supplemental graph fixtures from
`876b5b4` named in its inventory, contains 54 `ascii` graph fixtures, 25 `extended-chars` graph
fixtures, 12 `sequence` fixtures, and 5 `sequence-ascii` fixtures. The graph gate currently keeps
45 fixtures as an exact-output subset and names 34 deterministic Dagre/route/compound differences
that must still render and retain their semantics. Copied bytes are never rewritten to match local
output. Its 3 `multibyte` examples for accented Latin, Greek, and Cyrillic labels are useful semantic
evidence, but they are not admitted as byte-level fixtures: `merman-ascii` preserves the labels
readably while intentionally not copying the upstream LR label spacing byte-for-byte.

The ignored `repo-ref/mermaid-ascii` checkout inspected at
`b1b35f67d6a5dd0699ccfc968c00a763db573076` has added ER parsing, attribute tables, cardinalities,
and routed relationships since the pinned copied-source revision. No ER fixture from that moving
checkout is part of the tracked copied inventory, so those additions do not silently change Merman's
support or evidence boundary.

The 137-path moving fixture delta and its validity, admission, feature, and evidence dispositions
are tracked in `ASCII_MOVING_REFERENCE_MANIFEST.md`. Release CI does not need either reference
checkout to verify that inventory.

The `beautiful-mermaid` reference revision `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488` has 63 `ascii` fixtures and 37
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
support/gap documentation. Public metadata points to tracked inventories, tests, and documentation,
never to the ignored `repo-ref/` checkout. Prior-art references do not authorize byte-for-byte
expected output.

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
- Class/ER fixtures are local semantic fixtures by default. The pinned `mermaid-ascii` source does
  not cover those families; its moving local checkout now covers ER, but that post-snapshot work has
  not been admitted into the tracked copied inventory. `beautiful-mermaid` remains capability prior
  art rather than an official output oracle.
  Admit routed-grid Class/ER cases when the topology remains readable; admit structured
  relation-summary cases when dense crossings, port-fit failures, or route/overlay collisions make
  the honest terminal view a summary. Resource limits remain structured hard errors.
- Prefer semantic assertions for local fixtures: visible labels, direction relationships, grouping,
  routing reachability, unsupported-feature diagnostics, and absence of leaked implementation ids.
  Use exact ASCII snapshots only when the shape itself is the behavior under review.
- Reject a reference fixture as an oracle when it depends on a known reference shortcut, a browser
  rendering artifact, or terminal choices that are not implied by Mermaid semantics.

## Family Comparison

| Family | `mermaid-ascii` | `beautiful-mermaid` | `merman-ascii` | Readout |
| --- | --- | --- | --- | --- |
| Flowchart / graph | Exact copied-fixture parity for the narrow graph corpus, with LR/TD/TB routing and a small parser surface. | Broader graph ASCII ideas, ampersand fan-in/fan-out examples, richer shape handling, disconnected subgraph non-overlap checks, and more styling hooks, but `RL` is approximated as `LR`. | Merman preserves a broader typed surface: true `BT`/`RL`, Dagre-compatible ranking, first-parent compound ownership, boundary-aware subgraphs, scene-level occupancy, independent point/circle/cross/open endpoint markers, invisible constraints, explicit dispositions for every pinned Mermaid 11.16.1 shape name, distinct common decorated shapes, multiline labels, color roles, and terminal resource/text safety. It remains Partial for browser-only metadata, unimplemented uncommon geometry, arbitrary dense-route candidates, and mixed-stroke crossings. | Keep `mermaid-ascii` for compact routing evidence and `beautiful-mermaid` for UI ideas, but use pinned Mermaid semantics and executable terminal recovery gates as the product oracle. |
| Sequence | Exact copied-fixture parity for a compact sequence corpus. | Much broader parser/layout coverage, including notes, blocks, theming, and ASCII/Unicode variants. | Typed sequence support is already beyond the narrow reference: activations, create/destroy, boxes with inner padding, control blocks, mirror actors, and color roles all exist. | Remaining work is mostly layout polish and boundary tightening, not parser rescue. |
| Class | Not part of the pinned reference scope. | Full class parser/layout/ASCII, with compartments, annotations, multiline labels, and arrow-direction handling. | Supported subset through the shared `relation_graph` seam, with four terminal directions, independent source/target markers, endpoint cardinalities, annotation and method semantic probes, multiline relationship labels, self-relation loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, spanning routes, cyclic reverse-span lanes, structured crossing/port-fit/route/overlay-collision relation-summary fallback, dense multiline local semantic fixtures, and typed role colors. | Extend from typed relation facts, not from parser shape. |
| ER | The pinned copied-source revision has no ER renderer. The moving ignored checkout at `b1b35f67` now has entity tables, crow's-foot cardinalities, aliases, self-relations, and routed labels, but no ER fixture from it is an admitted oracle. | Full ER parser/layout/ASCII, including crow's foot notation, multiline relationship labels, and attribute sections. | Supported subset through the same `relation_graph` seam, with four terminal directions, entity boxes, attributes plus key markers, cardinality markers including Mermaid's parent diamond, multiline relationship labels, self-relationship loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, cyclic reverse-span lanes, structured crossing/port-fit/route/overlay-collision relation-summary fallback, and dense multiline local semantic fixtures. | Relation layout is the shared seam; cardinality and relationship identity stay family-specific. Moving reference code remains discovery evidence until explicitly pinned and inventoried. |
| State | Not part of the reference scope. | State diagram support rides the broader ASCII pipeline and gives useful layout ideas. | Supported subset with start/end, fork/join/choice, notes, composite states, divider regions, and role colors. | Keep state honest to the typed model; do not try to copy browser shapes literally. |
| XYChart | Not part of the reference scope. | Full xychart ASCII/SVG family, including legends, tooltips, and CSS-variable-driven palette behavior. | One typed terminal plan preserves model-owned x/y samples, titles, point labels, band/linear and reversed/degenerate ranges, grouped vertical/horizontal bars, missing-sample path gaps, connected horizontal lines, per-series topology-resolved ASCII/Unicode corners and crossings, mixed series, scale-aware ticks, exact data disclosure, display policy, configurable extents, terminal-safe text, and typed resource limits. Parser-produced x coordinates derive from the typed axis/category domain and sample order. | This is a stronger semantic and operational contract than copying browser hover behavior; the remaining residual is cross-series same-cell ownership after terminal quantization. |
| Mindmap / TreeView | Not part of the reference scope. | Broader mindmap/tree examples can suggest readable outline shapes. | Compact structured-text hierarchy outlines with preserved order and wrapped labels. | Keep the output readable and compact; do not imitate browser geometry. |
| Timeline / Gantt | Not part of the reference scope. | Broader schedule renderers can suggest readable summary patterns. | Readable rows that preserve sections, tasks, spans, and flags. | Favor honest text summaries over pseudo-graphs. |
| Journey / Kanban | Not part of the reference scope. | Broader board renderers can suggest grouping and actor/card metadata patterns. | Readable structured-text reports that preserve section order, actor order, and card metadata. | Keep the projection stable and compact. |
| Packet / GitGraph | Not part of the reference scope. | Broader process-diagram examples can suggest readable lane reports. | Readable structured-text reports that preserve ranges, parents, tags, commit order, and warnings. | Favor traceable text over decorative pseudo-graphs. |
| Railroad / Requirement / Ishikawa / Quadrant | Not part of the pinned ASCII oracle. | No reference output is accepted as Mermaid byte authority for these families. | Typed core models exist, but ASCII dispatch and exhaustive capability metadata remain explicitly Unsupported. | The tracked R34 report executes source-backed test-private spatial prototypes at 80/100/120 columns: each family has a concrete dense-case failure, while Ishikawa typical and Quadrant small/typical retain positive design signals. A future complete spatial vertical slice may reopen admission. |

## Intentional Differences

- True `RL` inversion is intentional. Treating `RL` as `LR` is a reference-implementation shortcut, not
  a product goal.
- Cyclic class and ER shapes should keep rendering through the layered planner when it can produce a
  readable route, and otherwise retain every relationship in a structured summary.
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
  keep explicit crossing, port-fit, route, and overlay boundaries. Resource budgets remain typed hard
  errors rather than summary selectors.
- XYChart dense-layout policy beyond the shipped compact plot and `values:` disclosure rows.
- Railroad, Requirement, Ishikawa, and Quadrant remain breadth candidates only after a new proposal
  supplies the complete spatial and width evidence required by
  `docs/rendering/ASCII_PHASE_GATE_REPORT.md`; summary-only output does not clear that gate.
