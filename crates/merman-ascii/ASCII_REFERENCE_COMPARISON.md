# ASCII Reference Comparison

Status: living comparison note
Last updated: 2026-11-15

This note records comparison evidence for the pinned upstream revisions used by `merman-ascii` and
notes what may be visible in the optional local reference checkouts:

- `AlexanderGrooff/mermaid-ascii` at copied-source commit `6fffb8e2714acab2c4cb41c78894fabbc62cee56`; `repo-ref/mermaid-ascii` may move beyond it.
- `lukilabs/beautiful-mermaid` at reference commit `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`; `repo-ref/beautiful-mermaid` is research-only.
- `xai-org/grok-build` at reference commit `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`; its first-party terminal module is Apache-2.0 and remains a behavior/presentation reference. Its separate MIT SVG component does not license that terminal module.

It is not a support gate. The shipped support matrices and gap registry remain the authoritative
product boundary.

## Short Read

- The pinned `mermaid-ascii` source is the narrow copied-fixture oracle for graph and sequence text
  output. The ignored local checkout inspected at `b1b35f67d6a5dd0699ccfc968c00a763db573076`
  also contains a newer ER renderer, but that post-snapshot code is discovery evidence only.
- `beautiful-mermaid` is the broad reference. It covers more families and more terminal styling
  ideas, but it is not always semantically faithful to Mermaid.
- Grok provides compact terminal graph examples with shared trunks and endpoint markers. Its
  truncation and group-endpoint abstraction require separate information checks; smaller output
  is not by itself evidence that a reader can recover the same graph.
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
  into one relation scene. A strict planar K2×2 four-node/four-edge cycle has a targeted bounded
  layout with four disjoint routes and declaration-order-stable geometry. Other dense, crossing, or
  collision-prone topologies remain eligible for the lossless Summary path. Summary reasons are
  preserved at that seam for direct topology-policy tests.
- The SVG compare CLI keeps per-diagram entrypoints, but common fixture discovery, upstream/local
  SVG loading, DOM checks, local SVG writing, and result sections belong to the shared `xtask`
  compare harness. Adapter code should supply render-specific policy, not reimplement the harness.

## Terminal Presentation Floor

Terminal graph presentation is best effort. A finite, non-zoomable terminal cannot display every
graph as continuous artwork. For the common supported comparison corpus, the refactor targets
comprehension at least as good as the best qualified Grok or mermaid-ascii result. This is a
per-case acceptance target, not a claim that the current renderer already meets it or that one
algorithm dominates every possible graph.

The original inputs and reading questions live in
[`tests/testdata/local-semantic/flowchart/presentation/`](tests/testdata/local-semantic/flowchart/presentation/README.md).
They are independent of copied upstream fixtures. They cover shared branches/merges, feedback,
member endpoints, distinguishing label suffixes, CJK text, and narrow/tall output. The first
capture compares 13 inputs with native unbounded output and an explicit 80-column request across
Merman, Grok's original terminal module, and the moving mermaid-ascii reference named below.

Selected measured output dimensions are **columns × rows**:

| Input / request | Merman | Grok | mermaid-ascii |
| --- | --- | --- | --- |
| `five-edge`, unbounded | 35 × 8, diagram | 32 × 6, diagram | 25 × 15, diagram |
| `fork-merge`, unbounded | 26 × 25, diagram | 21 × 13, diagram | 24 × 25, diagram |
| `fan-out-12-lr`, unbounded | 31 × 115, diagram | 28 × 48, diagram | 29 × 115, diagram |
| `fan-in-12-lr`, unbounded | 28 × 117, diagram | 25 × 48, diagram | 26 × 115, diagram |
| `fan-out-16-td`, 80 columns | 41 × 640, model-field fallback | 77 × 20, source frame | 171 × 11, over-width diagram |
| `long-labels`, 80 columns | 69 × 205, model-field fallback | 80 × 12, source frame | 225 × 11, over-width diagram |

The Merman code baseline is `f09aa799d6f42a2b84fc392508b7a9b3aaf94935`, built with defaults off
and `ascii,diagram-flowchart,diagram-state,diagram-swimlane`. Captures use plain Unicode output
and the Interactive resource profile. Bounded requests use Auto with Fallback. Grok compiles its
unmodified `xai-grok-markdown/src/mermaid.rs` in a local adapter with the upstream lock's direct
`ratatui 0.29.0` and `unicode-width 0.2.0` versions; backend features are unnecessary for its plain
lines. mermaid-ascii is built unmodified at `5a9df1560871e1ef8037c052b3b5d87de503bc9e` with its
native default Unicode settings and numeric width request. This moving reference does not replace
the existing copied-fixture pin. Common measurements use `unicode-width 0.2.0`; raw text, input
hashes, binary hashes, and local adapters remain under ignored `target/`.

The output kinds in the table matter. A source frame is not a diagram; an over-width result does
not satisfy the request; a full model-field listing is not automatically useful terminal
presentation. Dimensions and successful exit codes do not qualify information correctness.
For example, Grok truncates both targets in `distinct-suffixes` before the distinguishing
PRODUCTION/STAGING text. That smaller result cannot set a complete-information floor for this
input.

Reading the captured outputs before consulting their sources also identifies two limits of
geometric comparison. Grok emits identical output for the two group-member inputs even though
one connects Worker to Broker and the other Ingress to Store. In `feedback`, Merman's separate
return lanes preserve the two condition/endpoint triples; Grok's label placement and
mermaid-ascii's multiple-source/multiple-target bus make their pairing uncertain. These examples
qualify individual reference results rather than establishing a renderer-wide ranking.

The refactor may share compatible common-source or common-target trunks and arrows, simplify
expensive outlines with retained type cues, place full labels behind unique visible keys, and
replace difficult long connectors with named continuations. A separate arrowhead per semantic
edge is not an information requirement. Mixed marker kinds, branch conditions, multiplicity,
and actual member endpoints must remain understandable. Shared geometry must not imply unrelated
source/target pairs. Partial overviews must identify their scope and provide an available detail
route; they cannot pass as complete diagrams.

These are comparison and refactoring criteria. Existing production behavior and support gates
remain as documented by the support matrix until their implementation and tests change. The
comparison subset is not full State/Swimlane, all-direction, marker, generated, or resource
acceptance. It also does not require reference source or binaries in release CI.
The historical notes below describe the existing implementation and its regressions; independent
arrowhead counts are not a permanent requirement for a future unambiguous shared presentation.

Terminal layer ordering and independent node sizing have their own semantic and resource tests.
The Issue #53 viewport characterization changes from 74×57 to 76×57 in Canonical and from
56×67 to 57×67 in Compact, retaining the same fit classifications at 60/80/100/120 columns.
Those small width increases are recorded tradeoffs, not comparison-floor wins. The frozen gallery
above remains the pre-refactor baseline. Layer alignment alone does not resolve tall fan-outs,
shared terminal capacity, or the full-field width fallback; the common-case presentation floor
still requires joint allocation and route reconsideration.

Compatible common-source branches now retain an explicit shared-channel allocation through route
commitment and bounded replay. In the original unbounded `fan-out-12-lr` input, the current output
is 29×115 with one trunk and private target branches, compared with the frozen 31×115 baseline's
independent wrapping wires. The unchanged 115-row height remains a presentation gap against Grok's
qualified 28×48 result. The 80-column sixteen-way and long-label requests still emit the existing
full-field fallback; channel improvements do not establish the complete comparison floor.

## Reference Corpus Snapshot

The 2026-11-14 routing follow-up inspected the optional `mermaid-ascii` checkout at
`5a9df1560871e1ef8037c052b3b5d87de503bc9e`. Its
`backlink_with_short_y_padding.txt` contains the five-edge graph `A -> B`, `A -> C`, `B -> C`,
`B -> D`, `D -> C`, but the copied output shows four arrowheads after sharing the merge terminal.
Merman keeps the DAG ranks and all five independent heads instead of copying that layout. Local
regressions additionally cover four-to-six-way merges, six-way branch/merge graphs, mixed strokes,
edge-order permutations, wide labels, and long labeled feedback in Canonical and Compact output.
The improvements remain implementation policy rather than a new byte-level reference oracle.
The contour follow-up replaces fixed center-neighbor sampling with all shape-valid non-corner
contacts, and keeps port pairs lazy rather than materializing their Cartesian product. Local tests
compare the generated priority order with exhaustive sorting and protect cancellation, work limits,
diamond restrictions, and recovery after the three central contacts are occupied. Covered merges
now include eight and twelve incoming edges at zero preferred padding; two-ended fan-out through
fourteen edges checks source-marker symmetry. Unused alternate marker positions admit transverse
crossings without weakening protection of actual markers or primary terminals. Later marker
allocation cannot cover those crossings. Branch clearance follows the actually applied local
direction and reserves a real separator outside endpoint content. Neither reference supplies an
independent-marker oracle for arbitrary degree, and dense marked fan-out remains bounded.
Boundary endpoints reserve exterior transit space when marked demand exceeds their reachable
shape-valid contacts. Complete scene translation preserves ranks, node sizes, compound frames,
and the coarse-grid origin. Different branches at the same endpoint cannot form an adjacent
pointing-axis marker chain; same-pair parallel edges retain their independent contiguous markers.
The expanded search also exposed repeated futile searches through sealed frame approaches and
quadratic duplicate scans while generating label positions. Recovery now traces required straight
corridors before opening its path frontier, and label-position deduplication preserves the ordered
vector with an operation-local coordinate set. Existing mixed-feedback and nested endpoint tests
retain the unchanged Interactive work ceiling; these are resource-bound repairs, not timing claims.

Compound follow-up regressions wrap the same five-edge graph in single and nested subgraphs,
exercise perpendicular local directions, and cover cross-sibling merges and labeled feedback in
all four root directions. Coarse-grid and terminal-cell searches share frame containment and
transverse crossing rules. Obstructed aligned edges reserve an interior bypass lane in the nearest
common container, and routes preserve the title's horizontal reading gap. Assertions check actual
target contacts and complete frame geometry rather than accepting arrow counts alone. Arbitrary
dense compound scenes remain a bounded support limitation.

Nested sibling-chain coverage also protects parent placement: child subgraphs reserve their
complete member spans rather than a fixed node-sized placeholder. Moving unrelated nodes during
an incomplete local-direction pass could split a previously placed child and make its frame
enclose a sibling; local placement now moves complete children before shared frame spacing.

Container endpoint regressions extend that evidence to fan-in, fan-out, labeled parallel edges,
feedback, nested endpoints, and ancestor/member connections. Exterior routes stop at the group
frame without entering its member space; ancestor/member routes and labels use the frame's
interior. Independent target contacts, label ownership, and dual endpoint marker berths remain
explicit assertions. Coarse member-envelope separation and terminal frame padding have distinct
owners, avoiding repeated group displacement and large blank offsets. Container self-loop
regressions additionally cover labeled parallel loops beside an exit, nested ownership, two-ended
markers, and mixed strokes across root directions and layout profiles. Terminal recovery selects
distinct exterior contacts and excludes frame corners. The graph/sequence reference does not own
State concurrency behavior; [State support](STATE_SUPPORT.md) defines that separate boundary.

Nested container loops reserve turning clearance in their immediate parent. Each loop protects
its label before subsequent searches; unlike predetermined node-loop lanes, searched container
loops need not share a side or defer label allocation until the batch is complete.

The route-label audit also adopts the reference mapper's useful ownership principle: preserve the
selected straight host through planning and painting. Grid and feedback labels retain that segment
instead of reducing it to a single placement hint. Local admission protects both the text and its
perpendicular gap to the host, preventing another route from separating a relocated label from its
edge. This applies before and after route commitment. Multiline side-lane candidates account for
the complete text height, and node-loop envelopes account for the widest parallel label. Local
regressions cover long parallel loops and multiline feedback at zero preferred graph padding;
the reference's string-based edge labels do not serve as a multiline oracle.

The tracked corpus based on `mermaid-ascii` commit
`6fffb8e2714acab2c4cb41c78894fabbc62cee56`, plus the two supplemental graph fixtures from
`876b5b4` named in its inventory, contains 54 `ascii` graph fixtures, 25 `extended-chars` graph
fixtures, 12 `sequence` fixtures, and 5 `sequence-ascii` fixtures. The graph gate currently keeps
30 fixtures as an exact-output subset, 7 local compound-frame correction snapshots, and 42 named
Dagre/route/terminal-placement differences that must still render and retain their semantics.
Terminal node dimensions follow their own labels instead of stretching to an unrelated node's
shared axis band. This intentionally changes the multiline-node and differently sized disconnected
chain cases. The correction snapshots also reflect terminal layer ordering; independent geometry
checks still require closed frames, containment, and actual target contacts. Copied bytes are never
rewritten to match local output. Its 3 `multibyte` examples for accented Latin, Greek, and Cyrillic labels are useful semantic
evidence, but they are not admitted as byte-level fixtures: `merman-ascii` preserves the labels
readably while intentionally not copying the upstream LR label spacing byte-for-byte.

The ignored `repo-ref/mermaid-ascii` checkout inspected at
`b1b35f67d6a5dd0699ccfc968c00a763db573076` has added ER parsing, attribute tables, cardinalities,
and routed relationships since the pinned copied-source revision. No ER fixture from that moving
checkout is part of the tracked copied inventory, so those additions do not silently change Merman's
support or evidence boundary.

The moving fixture delta and its validity, admission, feature, and evidence dispositions are tracked
in `ASCII_MOVING_REFERENCE_MANIFEST.md`. Release CI does not need either reference checkout to
verify that inventory.

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
  Admit routed-grid Class/ER cases when the topology remains readable. The strict planar K2×2
  four-node/four-edge case is an explicit local admission backed by Class/ER parser tests plus
  shared route geometry, declaration-order, and exact/N−1 resource tests; it does not admit arbitrary
  bounded topology. Admit structured
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
| Flowchart / graph | Exact copied-fixture parity for the narrow graph corpus, with LR/TD/TB routing and a small parser surface. | Broader graph ASCII ideas, ampersand fan-in/fan-out examples, richer shape handling, disconnected subgraph non-overlap checks, and more styling hooks, but `RL` is approximated as `LR`. | Merman preserves a broader typed surface: true `BT`/`RL`, Dagre-compatible ranking, first-parent compound ownership, boundary-aware subgraphs, scene-level occupancy, independent point/circle/cross/open endpoint markers, invisible constraints, explicit dispositions for every pinned Mermaid 11.17.2 shape name, distinct common decorated shapes, multiline labels, color roles, and terminal resource/text safety. It remains Partial for browser-only metadata, unimplemented uncommon geometry, arbitrary dense-route candidates, and non-transverse mixed-stroke junctions. | Keep `mermaid-ascii` for compact routing evidence and `beautiful-mermaid` for UI ideas, but use pinned Mermaid semantics and executable terminal recovery gates as the product oracle. |
| Sequence | Exact copied-fixture parity for a compact sequence corpus. | Much broader parser/layout coverage, including notes, blocks, theming, and ASCII/Unicode variants. | Typed sequence support is already beyond the narrow reference: activations, create/destroy, boxes with inner padding, control blocks, mirror actors, and color roles all exist. | Remaining work is mostly layout polish and boundary tightening, not parser rescue. |
| Class | Not part of the pinned reference scope. | Full class parser/layout/ASCII, with compartments, annotations, multiline labels, and arrow-direction handling. | Supported subset through the shared `relation_graph` seam, with four terminal directions, independent source/target markers, endpoint cardinalities, annotation and method semantic probes, multiline relationship labels, self-relation loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, spanning routes, cyclic reverse-span lanes, a strict planar K2×2 bounded cycle with four disjoint routes, structured crossing/port-fit/route/overlay-collision relation-summary fallback for other complex scenes, dense multiline local semantic fixtures, and typed role colors. | Extend from typed relation facts, not from parser shape. The K2×2 path is topology-specific, not arbitrary bounded-graph support. |
| ER | The pinned copied-source revision has no ER renderer. The moving ignored checkout at `b1b35f67` now has entity tables, crow's-foot cardinalities, aliases, self-relations, and routed labels, but no ER fixture from it is an admitted oracle. | Full ER parser/layout/ASCII, including crow's foot notation, multiline relationship labels, and attribute sections. | Supported subset through the same `relation_graph` seam, with four terminal directions, entity boxes, attributes plus key markers, cardinality markers including Mermaid's parent diamond, multiline relationship labels, self-relationship loops, same-endpoint and bidirectional same-pair lanes, bounded iterative relation-layer sweeps, independent relation components, cyclic reverse-span lanes, a strict planar K2×2 bounded cycle with four disjoint routes, structured crossing/port-fit/route/overlay-collision relation-summary fallback for other complex scenes, and dense multiline local semantic fixtures. | Relation layout is the shared seam; cardinality and relationship identity stay family-specific. The K2×2 path is topology-specific, not arbitrary bounded-graph support. Moving reference code remains discovery evidence until explicitly pinned and inventoried. |
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
- The strict planar K2×2 four-edge cycle uses a local bounded cycle layout with four disjoint routes;
  this targeted exception does not change the Summary boundary for arbitrary dense or colliding
  Class/ER topology.
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
- Class and ER dense relation topologies beyond the admitted strict planar K2×2 cycle; new policy
  decisions should keep explicit crossing, port-fit, route, and overlay boundaries. Resource budgets
  remain typed hard errors rather than summary selectors.
- XYChart dense-layout policy beyond the shipped compact plot and `values:` disclosure rows.
- Railroad, Requirement, Ishikawa, and Quadrant remain breadth candidates only after a new proposal
  supplies the complete spatial and width evidence required by
  `docs/rendering/ASCII_PHASE_GATE_REPORT.md`; summary-only output does not clear that gate.
