# ASCII Reference Comparison

Status: living comparison note
Last updated: 2026-11-17

This note records comparison evidence for the pinned upstream revisions used by `merman-ascii` and
notes what may be visible in the optional local reference checkouts:

- `AlexanderGrooff/mermaid-ascii` at copied-source commit `6fffb8e2714acab2c4cb41c78894fabbc62cee56`; `repo-ref/mermaid-ascii` may move beyond it.
- `lukilabs/beautiful-mermaid` at reference commit `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`; `repo-ref/beautiful-mermaid` is research-only.
- `xai-org/grok-build` at reference commit `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`; its first-party terminal module is Apache-2.0 and remains a behavior/presentation reference. Its separate MIT SVG component does not license that terminal module.

It is not a support gate. The shipped support matrices and gap registry remain the authoritative
product boundary.

## Short Read

- The pinned `mermaid-ascii` source is the narrow copied-fixture oracle for graph and sequence text
  output. The ignored local checkout inspected at `5a9df1560871e1ef8037c052b3b5d87de503bc9e`
  also contains a newer ER renderer, but that post-snapshot code is discovery evidence only.
- `beautiful-mermaid` is the broad reference. It covers more families and more terminal styling
  ideas, but it is not always semantically faithful to Mermaid.
- Grok provides compact terminal graph examples with shared trunks and endpoint markers. Its
  truncation and group-endpoint abstraction require separate information checks; smaller output
  is not by itself evidence that a reader can recover the same graph.
- `merman-ascii` is the product renderer. It is typed-model driven, keeps the model boundary
  explicit, and should prefer honest terminal semantics over browser-shaped approximations.
- Mindmap, TreeView, Timeline, Gantt, Journey, Kanban, and GitGraph render as compact
  structured-text projections; keep those projections readable and headless rather than
  browser-shaped.
- Packet's Partial Diagrammatic contract uses a fixed two-cell bit scale for integer row widths
  in `1..=64`, preserving exact positions, gaps, row splits, and configured bit order/number
  display. Complete byte-framed labels and raw DB counts follow the plot; non-spatial models
  retain complete records and report StructuredText. The Packet admission note in the phase
  report records the complete-text comparison and verified 80/100/120-column results.
- The Phase 0-3 evidence and R34 admission decisions are tracked in
  `docs/rendering/ASCII_PHASE_GATE_REPORT.md`. Railroad, Ishikawa, and Quadrant remain
  Unsupported because each current prototype fails at least one required dense or typed-topology
  case at 80/100/120 columns; Ishikawa typical and Quadrant small/typical still demonstrate useful
  spatial signals. This is a current-cycle admission result, not a claim of permanent terminal
  unsuitability. Requirement has since reopened Partial Diagrammatic admission with a global
  shared-endpoint traceability graph and complete keyed records; its original tree prototype
  remains rejected.
- CJK/emoji terminal-cell coverage is semantic, not byte-exact: primitive cell ownership plus
  Flowchart, Sequence, Class, ER, and XYChart tests prove visibility/alignment without copying
  reference spacing.
- Class and ER relation output now deliberately shares the `relation_graph` seam. Adapters preserve
  Mermaid-family semantics, while shared relation planning owns bounded iterative layer-order sweeps,
  lane placement, routed grids, and structured crossing/port-fit/route/overlay-collision fallback.
  Independent relation subgraphs split before planning, so unrelated pairs do not force each other
  into one relation scene. Directed component ranks and bounded private channels now replace the
  strict K2×2 exception, with declaration-order-stable rings and direction-preserving DAG diamonds.
  Validated scenes compact redundant straight-channel space together, preserving box/text
  footprints, terminal and bend coordinates, and gaps between separate features.
  Other dense, crossing, or
  collision-prone topologies remain eligible for the lossless Summary path. Summary reasons are
  preserved at that seam for direct topology-policy tests.
- The SVG compare CLI keeps per-diagram entrypoints, but common fixture discovery, upstream/local
  SVG loading, DOM checks, local SVG writing, and result sections belong to the shared `xtask`
  compare harness. Adapter code should supply render-specific policy, not reimplement the harness.

## Static Routing Protection

The local `mermaid-ascii` source at
[`5a9df1560871e1ef8037c052b3b5d87de503bc9e`](https://github.com/AlexanderGrooff/mermaid-ascii/tree/5a9df1560871e1ef8037c052b3b5d87de503bc9e)
is an MIT-licensed research reference, separate from the copied-fixture pin above. Its
[`graph.go`](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/graph/graph.go#L32-L62)
stores a sparse node-occupancy grid, while
[`arrow.go`](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/graph/arrow.go#L64-L105)
searches that node-only grid. Ordinary route search does not commit existing edges as obstacles;
later character composition resolves their overlap. This is useful evidence for cheap static
obstacle queries, but does not establish independent relation ownership or label/marker clearance.

Merman independently compiles static protection into a bounded spatial hierarchy and shares it
across congestion trials. Dynamic route ownership, marker berths, labels, group-boundary rules,
and final validation remain separate and authoritative. This borrows the static/dynamic separation
without adopting the reference's node-only collision policy. The
[resource audit](../ASCII_RESOURCE_AND_PANIC_AUDIT.md#static-graph-protection)
owns the complexity and admission limits. No cross-renderer speed claim is made.

## Scene Planning and Explicit Recovery

The shared Flowchart/Swimlane/State scene planner now couples allocation with measured capacity
feedback. Rejected reservations identify actual canonical owners; atomic repair includes every
member of an affected shared channel. Stable semantic demand survives geometry changes, while
contacts and occupancy are rebuilt. The retained ranking engine may rebuild geometry at most
twice after the initial generation. Required feasibility and replacement work have separate finite
allowances from optional complete-scene quality work.

The reference informs the design without becoming a completeness claim. The MIT mermaid-ascii
node-occupancy search described above does not account for Merman's independent route, label,
marker, and frame ownership. Grok's compact presentation remains qualified by the complete-fact
reading checks below; its Apache-2.0 terminal implementation is not copied or translated.

Two constructive strict regressions demonstrate expanded diagrammatic capability independently
of fallback: one requires removing four actual label contributors together, and the other needs
measured reflow for 97 independent circle-marker contacts. The tests verify complete geometry and
relationship ownership; selecting text cannot satisfy either regression. The frozen thirteen-input
reading floor and all baseline-success cases in the 486-attempt generated oracle remain acceptance
controls rather than being resampled for the new algorithm.

The fixed 20-node/33-edge and 100-node/195-edge dense sources are maintained under
`crates/merman-ascii/tests/testdata/local-semantic/flowchart/scene_planning/`. Both now require
complete Strict spatial primaries under the unchanged Trusted Native work ceiling. Adaptive Plain
requests may still select a keyed graph document after other admitted misses. Those documents
preserve every endpoint, direction, label, marker, stroke, membership, and individually keyed
relationship. Width policy remains independent, and an unmeasured primary scene is reported as
unavailable under ASCII schema 4. Resource and cancellation errors remain errors in every mode.

Current-source native controls use the source-backed CLI operation, Canonical,
Plain, both ASCII and Unicode charsets, the unchanged Trusted Native policy, and a 10-second
operation control. At 80 cells, the reduced source returns a 24×115 Strict diagram and the original
source returns a 55×119 Strict diagram. The source bytes and complete scene requirements remain
frozen; crossing paint changes additionally require both axes to remain visible. These controls
establish completion and presentation outcomes, not a calibrated speed comparison or arbitrary
dense routing. Both diagrams are still tall; no height-selection policy is admitted.

These are bounded capability and information-preservation results. They do not show that Merman
dominates every reference graph, or that finite terminal artwork can represent arbitrary density.
The [support matrix](../ASCII_SUPPORT_MATRIX.md) owns policy admission and the
[resource audit](../ASCII_RESOURCE_AND_PANIC_AUDIT.md#graph-scene-planning)
owns the scheduling and accounting boundary.

## Extending Family Coverage

### Terminal relationship observation

Completion and Drawing label checks do not establish route continuity or endpoint ownership.
Merman's independent route oracle additionally checks actual crossing ink after painting, so a
preserved internal path cannot hide a missing visible axis. Unicode `╪` distinguishes an
independent crossing from joined `┼`; ASCII/CJK uses the conventional `+` with a disclosed
one-cell ambiguity. Adjacent cells carry each stroke, and label rows share a host column across
odd/even terminal widths and output mirrors. Independent vertical stems keep one blank column
beside a caption in both planning orders, so a long dotted-edge caption cannot appear attached
to a neighboring solid relationship. Its own host remains available.

Trident `ceb42fa4` has additional representation limits that must remain separate from product
failures. Its terminal graph observer has no qualified independent-crossing `╪` or dotted-turn
`·` glyph profile. Its Class member comparison treats equivalent abstract-classifier spellings
as different strings, and its endpoint observer requires cardinalities/markers to use its admitted
placement. The
anonymous-subgraph regression contract explicitly sets the default `arrow` marker, while the
observed ordinary arrow omits it; direct tuple comparison reports a mismatch even after tracing
the correct `a -> b` route. The label checker also matches the header `mindmap` inside preserved
`mindmapCircle` shape metadata. These are observer/contract boundaries, not permission to drop
metadata, alter source semantics, join independent paths, or claim an unassessed topology passed.

The executable controls remain family-owned. Generated small graphs and the fixed dense inputs
must preserve all endpoints, markers, labels, and painted continuations. Multiline labels and
eight parallel self-loops exercise both canonical axes; crossed branches retain their authored
TD scene. Both structural character sets are covered. The additional LR branch variant already
has a bounded planning miss at the baseline and does not acquire a spatial-success claim from a
paint or clearance change. Exact reference bytes remain immutable; unfamiliar output is
reported separately from admitted reference parity.

### Additional family mechanisms

The research checkout at `5a9df1560871e1ef8037c052b3b5d87de503bc9e` has three explicit renderer
adapters: Graph, Sequence, and ER. Its
[`DiagramFactory`](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/render/diagram.go#L13)
routes other inputs to the graph parser, which requires a `graph` or `flowchart` declaration;
this is not support for State, Kanban, Block, C4, Architecture, Usecase, Agentflow, or Requirement. The moving ER parser
explicitly [rejects subgraphs](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/er/parser.go#L181).
These omissions distinguish transferable layout techniques from solved family semantics.

Two reference mechanisms remain useful. The ER layout
[measures boxes, endpoint degree, and label width before placing entities](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/er/layout.go#L100),
then reserves private relationship lanes. Reserving that capacity early is useful; globally
allocating one lane per relationship to every gutter is not a compactness target. The Graph
[fan-out pass](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/graph/fanout.go#L9)
coordinates branch placement and label clearance, but only for a restricted TD, ungrouped,
compatible-arrow case. It is evidence for jointly measured channels, not a general compound
router. Merman's existing scene planner already owns the broader routing contract.

The reference graph parser extracts labels from several delimiter forms without retaining their
shape kind. It also [maps RL to LR and BT to TD](https://github.com/AlexanderGrooff/mermaid-ascii/blob/5a9df1560871e1ef8037c052b3b5d87de503bc9e/pkg/graph/parse.go#L609).
Those simplifications do not establish complete-information parity for Merman. Future terminal
approximations should preserve the authored type, label, endpoint identity, and direction, with
their visible cues or keyed details measured before routing.

Typed admission repairs and family-owned terminal projections have separate evidence. Shared
graph mechanisms do not establish another family's semantics:

| Area | Owner | Current boundary or next acceptance condition |
| --- | --- | --- |
| Agentflow | `agentflow.rs`, `output/agentflow.rs` | Family-owned role contours and sequence/reference/failure semantics share graph routing. Expanded flow ownership is preserved; rich metadata, collapsed views, source diagnostics and ambiguous containment select honest complete keyed records. Neither the pinned nor current `mermaid-ascii` provides an Agentflow renderer or semantic quality oracle. See `AGENTFLOW_SUPPORT.md`. |
| Requirement | `requirement.rs`, `output/requirement.rs` | A global shared-endpoint graph plus complete keyed records retains all seven relationship kinds and requirement/element fields. Contains keeps its source marker and headless target. Ambiguous names and unresolved endpoints remain explicit records. The historical R34 convergence failure is resolved; dense routing remains bounded. See `REQUIREMENT_SUPPORT.md`. |
| Pie | `pie.rs`, `output/pie.rs` | Explicit shared-scale percentage bars retain canonical order, tiny/zero categories, exact values, labels, and metadata. Undefined distributions use complete StructuredText primaries. No Pie renderer exists in the copied or inspected moving `mermaid-ascii`; pinned Mermaid supplies source semantics, while independent geometry tests own terminal quality. See `PIE_SUPPORT.md`. |
| Uncommon Flowchart shapes | `graph/shape.rs`, `graph/adapter.rs` | Admitted known aliases use a visible type compartment measured before contour sizing and routing. Unknown, cross-family, and remaining BPMN shapes retain precise diagnostics. |
| Flowchart icon/image nodes | Family-owned measured node presentation | Complete labels, asset identities, form, position, constraint, and numeric dimensions are retained as terminal-safe text. Quoted whitespace survives wrapping and mirroring. Assets are not fetched; fixed-label control shapes reject metadata they cannot display. Aggregate temporary text counts against output admission. |
| Flat cyclic Flowcharts | `graph/layout/feedback.rs`, `graph/layout/grid/rank.rs` | After a routing miss with no further capacity feedback, visible-edge scenes without authored side constraints may use SCC condensation ranks and transverse member lanes. No node or relationship is contracted. The frozen 100-node/195-edge and 20-node/33-edge inputs require complete spatial scenes; the independent route oracle checks every original contribution in both canonical axes and structural character sets. Two smaller cyclic controls preserve complete 50-column results and edge labels through existing capacity recovery. Rebuild and work allowances remain unchanged. |
| ER group endpoints | `er/render/group.rs`, `output/relation.rs` | Nested and empty group frames distinguish authored membership keys from generated entity-route IDs. Entity/group endpoints, both cardinalities, stroke, and role survive. Deep-member facades name actual endpoints; ancestor/member relations use complete summaries. Primary geometry and structured width fallback report their actual projections. |
| Repeated Kanban card IDs | Ordered occurrence-based card records | Every occurrence retains its authored ID, order, column, label, and metadata. Duplicate group IDs remain an ownership diagnostic; group/card spelling collisions do not discard cards. |
| State concurrent regions | Shared graph capacity planning and geometry-local label classification | Cyclic compound-scope return demand is measured before route search. Root returns and self loops keep existing exterior envelopes; recovery remains available. Exact labels, actual cross-region endpoints, operation accounting, and finite search diagnostics are preserved. |
| Usecase | `usecase/adapter.rs`, `usecase/validation.rs`, `usecase/json_table.rs`, `output/usecase.rs` | Family-owned role labels and contours distinguish actors, cases, flat boundaries/packages, and attached notes. Include, extend, and generalization retain explicit kinds, labels, independent endpoint markers, and direction. Ordered JSON tables retain typed indexed paths, quoted literal values, UTF-16 identities, and signed infinity metadata; nested direct-model boundaries remain diagnostics. See `USECASE_SUPPORT.md`. |
| Block | `block/pack.rs`, `block/validate.rs`, `graph/positioned.rs`, `output/block.rs` | Family-owned integer packing preserves columns, spans, source order, spaces, and nested frames. The positioned graph seam shares routing and painting without invoking Dagre or moving authored blocks. Dense routing misses remain diagnostics; unsupported numeric geometry and ambiguous bindings are rejected. See `BLOCK_SUPPORT.md`. |
| C4 | `c4/pack.rs`, `c4/validate.rs`, `graph/positioned.rs`, `output/c4.rs` | Five views share a borrowed model and preserve declaration-order rows, nested frames, shape-first endpoints, and independent arrowheads. Role/name headings and short E keys keep the overview small; technology, environment type, descriptions, styles, and raw values remain complete records. Invalid detached models use StructuredText primaries; Strict routing failures remain errors. Pinned Mermaid supplies semantics; neither copied nor inspected moving `mermaid-ascii` supplies a C4 byte or quality oracle. See [C4 support](C4_SUPPORT.md). |
| Architecture | `architecture`, `graph/positioned.rs`, `output/architecture.rs` | A borrowed canonical model supplies services, junctions, groups, and relationships. Spatial output must preserve declared sides, independent arrowheads, service-anchored group contacts, and ordered Row/Column constraints. Invalid or conflicting detached geometry uses complete StructuredText primaries; Strict routing and operation failures remain errors. See [Architecture support](ARCHITECTURE_SUPPORT.md). |

The exact three-region Trident State source is retained as a parser-backed Trusted Native
regression with all six full condition occurrences, six state labels, four starts, and ten heads.
Attribution identified exhaustive searches in two insufficient layout generations before measured
return-space recovery made the final generation easy to route. Preparing compound return capacity
before searching avoids those generations. This is a deterministic work/admission improvement;
it does not establish a calibrated latency improvement or arbitrary-graph success. No default
ceiling is raised and no authored condition is shortened.
The exact native CLI control admits 700,138 work units instead of 21,764,888 and emits 71×79
instead of 71×81. Before displacing existing routes, the planner now tries a bounded detour
against unchanged occupancy, splitting remaining preparation/allocation credit with repair.
This restores the single-region control from 56×32 to 20×32, with complete labels and heads;
its exact public work threshold falls from 586,045 to 172,600. Conservative spacing can still
widen layouts whose original route cannot be proved within the finite trial. These are bounded
tradeoffs, not universal compactness or reference superiority claims.

Usecase and Block have parser-backed small, typical, CJK, and dense controls at 80/100/120 cells,
with both structural character sets and Unicode/CJK width profiles. Complete keyed reading records
are selected only after a valid spatial primary exceeds an explicit Plain viewport using Fallback.
Existing reading-floor and resource/cancellation tests remain acceptance controls. Width compliance,
complete facts, recoverable relationships, and correct projection reporting matter more than a
successful exit or a smaller rectangle. Neither family gains Compact, Auto, or adaptive graph
presentation admission. Unsupported semantics, fixed-grid routing misses, and resource errors do
not become generic successful fallbacks. These are independent Merman implementations: neither
reference supplies a Usecase or Block renderer to port or a family-quality oracle to match.

The reference is MIT-licensed. Substantial copied code or fixtures require its copyright and
permission notice; researching the mechanisms does not change Merman's copied-fixture pin.
Grok's first-party terminal implementation remains Apache-2.0 presentation research, not source
to copy or translate in this work.

## Terminal Visual Direction

Grok's pinned terminal implementation provides useful examples of compact text and restrained
host styling. Its ordinary mask-based wires use square turns; rounded turns appear explicitly in
Flowchart self-loops and Sequence self-messages. Node rectangles use square corners, while both
Round and Diamond map to rounded boxes. Group frames are solid rectangles with a title inserted
into the top border. See the pinned
[wire masks](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-markdown/src/mermaid.rs#L1730),
[frames](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-markdown/src/mermaid.rs#L2273),
and [node/self-loop drawing](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-markdown/src/mermaid.rs#L3052).
Its text wrapping defaults to 24 columns and four lines. These density limits are presentation
evidence, not permission to truncate Merman's complete labels. Grok maps visual classes to host
Markdown styles rather than prescribing a fixed terminal RGB palette.

Merman uses a distinct, independently implemented graph grammar: label-sized authored node
contours, rounded turns for every ordinary Unicode wire, square branch junctions and frame
contacts, solid arrowheads, and complete labels. This separates a path from a rectangular object
without spending extra cells or requiring color. Shape-specific node outlines, dotted and thick
edge meanings, ASCII/CJK structural fallback, container ownership, and width/resource policy
remain authoritative. The crate's [visual-style guide](../../../crates/merman-ascii/README.md#terminal-visual-style) owns these
defaults. Two copied Unicode self-loops now have strict local style expectations; their original
fixture bytes and the full 79-case denominator remain intact.

Group titles use a centered block with left-aligned lines. Unlike centering each row separately,
this preserves a common reading start for multiline headings. A flush-left title was also tested:
it obstructed the direct `Start → Entry` boundary route in the nested-direction input and forced
side contacts. Keeping the single-line envelope preserves that useful transit space. Title drawing
and routing protection now share coordinates, including RL alignment and the upright BT row order;
the full copied-fixture denominator and single-line expectations remain unchanged.

Dashed containers were considered as a second hierarchy cue. Solid frames remain the default:
ordinary dotted relationships and State concurrency separators already use the dotted strokes.
Embedding the title in the top border would also consume real boundary contacts and requires its
own multiline/port-capacity design. Neither rule is adopted merely for visual variety. The terminal
Grok source remains Apache-2.0 research material; these refinements do not copy or translate its
implementation.

## Terminal Presentation Floor

Terminal graph presentation is best effort. A finite, non-zoomable terminal cannot display every
graph as continuous artwork. For the common supported comparison corpus, the planner is evaluated for
comprehension at least as good as the best qualified Grok or mermaid-ascii result. This is a
per-case acceptance target, not a claim that one
algorithm dominates every possible graph.

The original inputs and reading questions live in
[`crates/merman-ascii/tests/testdata/local-semantic/flowchart/presentation/`](../../../crates/merman-ascii/tests/testdata/local-semantic/flowchart/presentation/README.md).
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

## Terminal planner comparison

The frozen table above retains the pre-refactor baseline. Current terminal planning uses whole-scene
endpoint/label demand, independent contours, jointly allocated compatible channels, and reversible
local conflict repair. Actual-contact channels and compressed orthogonal search replace the old
coarse route selectors and emergency detours. Source and target heads may share their precise allocated
berth; incompatible markers, conditions, and unrelated endpoint pairs remain distinct.

Current measured output dimensions, including all detail rows, are **columns × rows**:

| Input / request | Current Merman | Frozen Merman | Qualified comparison |
| --- | --- | --- | --- |
| `five-edge`, unbounded | 35 × 6, diagram | 35 × 8, diagram | Grok: 32 × 6; all five direct relationships remain traceable |
| `fork-merge`, unbounded | 22 × 15, diagram | 26 × 25, diagram | Grok: 21 × 13; short explicit branches and merge |
| `fan-out-12-lr`, unbounded | 29 × 47, diagram | 31 × 115, diagram | Grok: 28 × 48; twelve private target branches |
| `fan-in-12-lr`, unbounded | 26 × 47, diagram | 28 × 117, diagram | Grok: 25 × 48; twelve sources and one explicit sink |
| `fan-out-16-td`, 80 columns | 62 × 19, graph document | 41 × 640, model-field fallback | Grok: 77 × 20 source frame; first-use target declarations avoid a second lookup table |
| `long-labels`, 80 columns | 79 × 11, graph document | 69 × 205, model-field fallback | Grok: 80 × 12 source frame; both complete conditions and all endpoints remain recoverable |

The width matrix is executable in `crates/merman-ascii/tests/presentation_floor.rs`, using the same original 13 inputs
at unbounded/60/80/100/120 cells, ASCII/Unicode structure, and Unicode/CJK width profiles. At those
four bounded widths, sixteen-way TD uses 20/19/19/19 rows, and long-label output uses 13/11/11/9.
The fallback declares each Flowchart node with its first relationship, then refers to its key;
complete labels may continue on the following physical row. Group membership, real member
endpoints, types, marker/stroke distinctions, and per-edge conditions remain explicit. The document
does not reproduce a diagram's spatial direction, and reports `structured_text` and
`presentation_only` through the existing output contract.

Geometry alone remains insufficient. The `feedback` comparison keeps `retry` on Validate→Parse
and `failed` on Write→Input. Conditions cannot float away from their host to claim a smaller area,
and a transverse crossing paints a horizontal overpass rather than a joined bus. The two
group-member examples retain different actual endpoints, where Grok's identical frame-level output
does not supply a qualified reference. Distinguishing PRODUCTION/STAGING suffixes remain complete.

Route admission reserves the forward backbone before feedback lanes. The `feedback` presentation
keeps Input→Parse→Validate→Write→Done straight, retains `retry` on Validate→Parse and `failed` on
Write→Input, and remains 78×7. The regression exercises all four directions, both character sets,
and reversed edge declarations. The existing generated oracle and complete-document presentation
floor remain required: a clearer backbone does not justify rejecting a previously supported cycle
or moving a condition off its connection.

Compound exit regressions coalesce consecutive unallocated rank bands after a local direction
override. Explicit spacing and edge-length requests remain allocated; expanding a separator
counts its retained default corridor once. Canonical Unicode output for `nested_direction_boundary`
changes from 32×33 to 32×28, `cjk_boundary_routes` from 40×41 to 40×28, and `boundary_label_lane`
from 31×26 to 31×20. `disconnected_subgraphs` retains 22×39 and its one-cell reading gutter.
Both Canonical and Compact are checked, including complete labels and the original group owners.

Legal transverse crossings contribute to path cost rather than outranking every possible detour.
The acyclic `multi_boundary_routes` case changes from 48×34 to 48×26 in Canonical and from 44×32
to 44×26 in Compact, without routing a forward edge beneath its target and returning upward.
Reciprocal routes may also cross transversely; shared collinear transit remains invalid.

Compound forward routing additionally resolves the edge's common explicit scope before selecting
its endpoint sides. In `multi_boundary_routes`, Validate→Publish stays on the internal LR chain,
while Publish→Success/Retry leave downward and enter their targets from above. In
`nested_direction_boundary` and its CJK counterpart, the terminal group exit now follows the root
TD direction instead of entering Done from the side. The local 46-output comparison against
`bae4152236b62e5a682a64fbf8dbf414cf2c1620` retains every width, height, and width-fallback outcome:
six outputs change route geometry, thirty change only ordinary wire-corner glyphs, and ten remain
identical. This is a finite presentation check, not a claim that forward contacts improve every
possible scene. Scope-direction tests also cover the mirrored BT root, a sealed Compact ingress,
and preferred contacts that must yield to independent alternate ports. Frame titles and node
reading margins remain hard constraints. Two independent ASCII compound snapshots now use
bottom exits and may contact a target away from its side center; their geometry checks cover the
complete non-corner target contour without changing the copied upstream files.

The independent generated oracle fixes 486 Interactive scene attempts and 3,042 visible edge
contributions, with up to eight nodes/twelve edges, nine topology categories, three edge orders,
and LR/TD plus normal/zero preferred gaps. Failures remain in the denominator. The historical
baseline-success set includes 54 ordered-chain attempts independently checked for continuity and
direction. Existing State/Swimlane, four-direction, marker, container, CJK, and cancellation tests
cover the additional family boundaries; this finite corpus is not an arbitrary-graph guarantee.

Issue #53 changes from the historical 74×57/56×67 characterization to 76×40 Canonical and 63×50
Compact. Compact narrows the scene while retaining visible turns, spending more area and blank
cells than current Canonical. That tradeoff is recorded explicitly rather than counted as a
universal density improvement. All original frozen reference captures remain unchanged.

### Accepted planner boundary

The shared-planner refactor is complete for the admitted Flowchart, State, and Swimlane subset.
The completion reading review used implementation `78b94dfe8ca2dc3a8cc8adf56101e90a90e83644` with
explicit Interactive resources. Twenty-four inputs cover the thirteen frozen Flowchart cases,
three additional nested/cross-boundary graphs, four State lifecycles/composites/notes, and four
Swimlane member/group-endpoint and long-label cases. All 480 native/60/80/100/120 requests across
ASCII/Unicode and Unicode/CJK width profiles succeeded within their requested widths. Failures
were retained in the attempted denominator. This finite review supplements the generated and
semantic oracles; success and dimensions alone do not prove arbitrary-graph readability.

State requests used Canonical; bounded Flowchart/Swimlane requests used Auto with Plain Fallback.
Of the 480 results, 416 remained diagrams and 64 used structured graph records. The reading review
retained separate branch conditions, forward/feedback identity, group versus member contacts,
concurrent State region separation, complete distinguishing suffixes, and CJK text. No reference
source or binary is required in CI. Raw reports, inputs, hashes, and the comparison gallery remain
under ignored `target/`.

The review explicitly accepts these remaining tradeoffs:

- State has no Compact or height policy. At that checkpoint its CJK connection lifecycle was a
  27×63 Canonical diagram. Its note/composite example used 60×48, 80×43 and 100×42 full structured
  records at bounded widths, versus a 119×43 diagram at 120 columns. The authored detail rows
  preserve more than the primary diagram displays and can require additional reading. A dotted
  separator can start the first concurrent region below the title band; readers must not infer
  the region count from separator count alone.
- The Swimlane handoff fits at 62×9. At 60 columns, Auto attempts a valid Compact candidate of the
  same width, retains Canonical on the tie, and produces 53×12 complete structured records. A
  compact request does not guarantee a narrower result.
- Dense compounds and marked endpoint demand can still exhaust bounded routing or fixed
  presentation capacity. Required search/capacity errors do not select width fallback and do not
  prove that another layout is impossible. Resource and cancellation errors remain terminal.

Future density, label-placement, or style improvements are separate iterations. New semantic loss,
unexplained regressions in the accepted corpus, or inaccurate diagnostics remain defects under the
[graph presentation boundary](../../../crates/merman-ascii/README.md#graph-presentation-boundary).

### Canonical density and detail wrapping

The 2026-11-15 follow-up against `8ed4469fbbb5a5ea65becd02c4a2ec18f92a9dc6` keeps State
Canonical-only while removing its legacy preferred blank node rows and five-row rank gaps.
The planner still enlarges contours and gaps for endpoints, complete labels and containers.
Explicit padding/gap builders retain their values, including the legacy numeric defaults.
Ungrouped independent contours share their rank center, avoiding offset side-note links when
the note is taller than its state. The same 480-request reading matrix succeeds with no width
violations, retaining 416 diagrams and 64 complete structured fallbacks.

The native Unicode/non-CJK State comparisons are:

| Case | Previous extent | Current extent |
| --- | ---: | ---: |
| Nested composites | 61×29 | 61×25 |
| Concurrent regions | 37×24 | 37×16 |
| Long condition and note | 119×43 | 119×30 |
| CJK connection lifecycle | 27×63 | 27×48 |

The CJK lifecycle retains all nine heads and its internal state order across both character
sets and width conventions. A separate five-contour forward/reverse lifecycle fits in 23 rows;
explicit legacy padding and gaps reproduce its 41-row layout. These are bounded presentation
checks rather than a maximum-height guarantee. Complete labels and endpoint demand can still
require large diagrams.

Rank centering also changes the independently checked Issue #53 Compact layout from 63×50 to
59×50, retaining seven heads, the dotted relation and every condition. It now fits the 60-column
viewport, while Canonical remains 76×40. The current whitespace figures and encoding checks live
in the [support matrix](../ASCII_SUPPORT_MATRIX.md#testing-policy).

Graph detail records now use word-aware indented continuation, including full State authored
documents, links and relations. Words can use the entire viewport when indentation would prevent
them fitting; longer atoms retain grapheme-safe hard wrapping. Independent record recovery checks
preserve original whitespace, control escapes, CJK/combined graphemes, repeated labels and each
authored owner. All complete semantic fallbacks retain data spaces under a trim request and report
`trimmed: false`; a non-graph Sequence property regression covers the same shared emission rule.
Repeated authored details remain visible because identical text can belong to distinct records.
No reference source or binary is required for these checks.

### Source-bound validation

The initial whole-scene routing measurements were bound to implementation commit
`65eb8bb48dc7d020b8e8341c29f7d2d031758e3b` (tree
`23fba7ba4dfd0dcb1a0d159f82242a62bb8a6d7a`). Rebuilding the defaults-off CLI with
`ascii,diagram-flowchart,diagram-state,diagram-swimlane` reproduced all 26 reviewed
unbounded/80-column outputs byte-for-byte. All 13 input hashes match the frozen reference gallery;
52 further captures reproduce the 60/80/100/120-column reports. The rebuilt CLI SHA-256 is
`79f6b6927174a19fc79365a66db6fcf0881d1d090fe247ce0f5fa91f36e9bc57`.

Settled-source checks passed 1,568 ASCII tests, 33 facade/CLI tests, 199 facade and binding-core
library tests, and the two representative UniFFI/WASM exact resource boundaries. These include
all 486 generated scenes and their 3,042 visible relationships. Strict ASCII all-target Clippy,
scoped formatting, the three isolated graph-family builds, the WASM target build, repository path
limits, and third-party license verification also passed. Copied upstream fixtures are unchanged.

The public `ascii_pipeline` benchmark was rebuilt from the same implementation with defaults off
and `all-diagrams,ascii`. On Windows 11, `x86_64-pc-windows-msvc`, Rust 1.100.0, and an Intel
i9-13900KF, short diagnostic runs used a one-second warmup, a two-second requested measurement,
and ten samples. Criterion extended the large case to approximately 6.4 seconds for ten iterations.
These are current-artifact measurements, not an adjacent-revision speedup or U25 performance
closeout claim. The output has intentionally changed from the frozen product baseline.

| Public operation / fixture | Estimate | Reported 95% interval | Output bytes |
| --- | --- | --- | --- |
| `ascii_end_to_end/flowchart_medium` | 4.8112 ms | 4.6847–4.9692 ms | 13,202 |
| `ascii_end_to_end/flowchart_large` | 809.78 ms | 751.73–867.84 ms | 75,429 |

Both exact invocations retained identical preflight/postflight output. Their plain ASCII output
SHA-256 values are respectively
`53019517e81ce2ba2a63eb93e041681c8dcf2c7425f462af2643184827c2402e` and
`2e28484b52fb3db21856c0836343dcde072b9b537dd26d119e02ba9df4c74bf7`.
The benchmark executable SHA-256 is
`738edeb19d241b52f56283febd7587a6629741f61573a4754c9b3ee6d04035c9`.
The maintained compiled-list verifier also passed all ten benchmark registrations and preflight
receipts. Reproduce these checks through the
[ASCII benchmark contract](../../performance/BENCHMARKING.md#ascii-semantic-depth-benchmark),
using `--exact ascii_end_to_end/flowchart_medium` or `--exact ascii_end_to_end/flowchart_large`
and `--warm-up-time 1 --measurement-time 2 --sample-size 10 --noplot` for the diagnostic timing.
Raw captures, samples, commands, and environment evidence remain under ignored `target/`.

### Graph document wrapping

The follow-up keeps node/relationship words and short type cues together when they fit the
continuation width, with two-space continuation indentation. For example, the 80-column long-label
case now retains `required` and `[diamond]` on complete physical rows. Very long tokens still use
grapheme-safe splitting, atomic visible escapes can use the full viewport, and original field
whitespace remains recoverable. This changes graph-document text bytes; the source-bound hashes
above remain the historical receipt for implementation commit `65eb8bb48`.

The existing presentation gate retains its complete-document row ceilings at all four widths.
Reader-level tests still recover every node, real endpoint, condition, type, and group, including
quoted record-like text, combining marks, emoji, and CJK labels. Exact byte/cell tests count the
added layout indentation, and cumulative work/cancellation checks cover the streaming lookahead.

## Reference Corpus Snapshot

The 2026-11-14 routing follow-up inspected the optional `mermaid-ascii` checkout at
`5a9df1560871e1ef8037c052b3b5d87de503bc9e`. Its
`backlink_with_short_y_padding.txt` contains the five-edge graph `A -> B`, `A -> C`, `B -> C`,
`B -> D`, `D -> C`, but the copied output shows four arrowheads after sharing the merge terminal.
The pre-refactor Merman baseline kept the DAG ranks and all five independent heads. The current
planner permits a shared head when its compatible merge retains all five direct relationships. Local
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
and open region spans. Different branches at the same endpoint cannot form an adjacent
pointing-axis marker chain; same-pair parallel edges retain their independent contiguous markers.
The expanded search also exposed repeated futile searches through sealed frame approaches and
quadratic duplicate scans while generating label positions. Recovery now traces required straight
corridors before opening its path frontier, and label-position deduplication preserves the ordered
vector with an operation-local coordinate set. Existing mixed-feedback and nested endpoint tests
retain the unchanged Interactive work ceiling; these are resource-bound repairs, not timing claims.

Compound follow-up regressions wrap the same five-edge graph in single and nested subgraphs,
exercise perpendicular local directions, and cover cross-sibling merges and labeled feedback in
all four root directions. Actual-contact candidates and compressed channel search share frame
containment and transverse crossing rules. Obstructed aligned edges reserve an interior bypass lane in the nearest
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
28 fixtures as an exact upstream-output subset, 2 strict local Unicode wire-style snapshots,
7 local compound-frame correction snapshots, and 42 named
Dagre/route/terminal-placement differences that must still render and retain their semantics.
Terminal node dimensions follow their own labels instead of stretching to an unrelated node's
shared axis band. This intentionally changes the multiline-node and differently sized disconnected
chain cases. The correction snapshots also reflect terminal layer ordering; independent geometry
checks still require closed frames, containment, and actual target contacts. Copied bytes are never
rewritten to match local output. Its 3 `multibyte` examples for accented Latin, Greek, and Cyrillic labels are useful semantic
evidence, but they are not admitted as byte-level fixtures: `merman-ascii` preserves the labels
readably while intentionally not copying the upstream LR label spacing byte-for-byte.

The ignored `repo-ref/mermaid-ascii` checkout inspected at
`5a9df1560871e1ef8037c052b3b5d87de503bc9e` has added ER parsing, attribute tables, cardinalities,
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
  clearer than a copied render. Those fixtures live under `crates/merman-ascii/tests/testdata/local-semantic/`, and they
  are intentionally outside the copied fixture inventory gate.
- Class/ER fixtures are local semantic fixtures by default. The pinned `mermaid-ascii` source does
  not cover those families; its moving local checkout now covers ER, but that post-snapshot work has
  not been admitted into the tracked copied inventory. `beautiful-mermaid` remains capability prior
  art rather than an official output oracle.
  Admit routed-grid Class/ER cases when the topology remains readable. Directed rings and DAG
  diamonds are local admissions backed by parser tests plus shared route geometry,
  declaration-order, and exact/N−1 resource tests; they do not admit arbitrary dense topology. Admit structured
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
| Class | Not part of the pinned reference scope. | Full class parser/layout/ASCII, with compartments, annotations, multiline labels, and arrow-direction handling. | The shared `relation_graph` seam preserves four directions, independent markers, endpoint labels, namespaces, annotations, methods, self-relations, typed colors, and independent components. Directed component ranks and bounded private channels handle admitted rings and DAG diamonds, with complete summaries when route or overlay admission fails. | Extend from typed relation facts and verify traceable contacts and labels. Bounded channel search does not guarantee arbitrary dense routing. |
| ER | The pinned copied-source revision has no ER renderer. The moving ignored checkout at `5a9df156` has entity tables, crow's-foot cardinalities, aliases, self-relations, and routed labels, but rejects subgraphs; it is not an admitted byte oracle. | Full ER parser/layout/ASCII, including crow's foot notation, multiline relationship labels, and attribute sections. | The same relation planner preserves four directions, entities and attributes, keys and comments, independent cardinalities including the parent diamond, multiline labels, identifying strokes, and self-relations. Directed ranks and private channels retain admitted rings and diamonds; unsafe scenes use complete summaries. | Relation geometry is shared; cardinality and relationship identity stay family-owned. Moving reference code remains discovery evidence until explicitly pinned and inventoried. |
| State | Not part of the reference scope. | State diagram support rides the broader ASCII pipeline and gives useful layout ideas. | Supported subset with start/end, fork/join/choice, notes, composite states, divider regions, and role colors. | Keep state honest to the typed model; do not try to copy browser shapes literally. |
| XYChart | Not part of the reference scope. | Full xychart ASCII/SVG family, including legends, tooltips, and CSS-variable-driven palette behavior. | One typed terminal plan preserves model-owned coordinates, grouped bars, missing-sample gaps, joint line topology, distinct series markers, shared sample cells, scale-aware ticks, and exact record-oriented disclosure after the plot. Measurement, painting, encodings, and resource admission consume the same plans. | Terminal quantization, marker reuse after six lines, dense bar/line overlap, and long individual fields still require exact records or viewport fallback. |
| Mindmap / TreeView | Not part of the reference scope. | Broader mindmap/tree examples can suggest readable outline shapes. | Compact structured-text hierarchy outlines with preserved order and wrapped labels. | Keep the output readable and compact; do not imitate browser geometry. |
| Timeline / Gantt | Not part of the reference scope. | Broader schedule renderers can suggest readable summary patterns. | Readable rows that preserve sections, tasks, spans, and flags. | Favor honest text summaries over pseudo-graphs. |
| Journey / Kanban | Not part of the reference scope. | Broader board renderers can suggest grouping and actor/card metadata patterns. | Readable structured-text reports that preserve section order, actor order, and card metadata. | Keep the projection stable and compact. |
| Packet | Not part of the reference scope. | No Packet byte-parity oracle is adopted. | Bounded Partial Diagrammatic contract: integer rows of 1..=64 bits at two terminal cells per bit, exact gaps and cross-row splitting, source-configured order/number display, and complete authored-label/raw-DB records after the plot. Non-spatial cases report StructuredText; borrowed models default to 32/ascending/showBits. | Narrow labels may use `*` with a legend and exact details; geometry never expands for text. This is a terminal admission bound, not a Mermaid syntax limit. Small/typical/dense cases pass all 80/100/120-column gates against the complete record baseline; the phase report records dimensions and vertical cost. |
| GitGraph | Not part of the reference scope. | Broader process-diagram examples can suggest readable lane reports. | Readable structured-text reports preserve parents, tags, commit order, and warnings. | Favor traceable text over decorative pseudo-graphs. |
| Requirement | Not part of the pinned ASCII oracle or inspected moving renderer. | No Requirement byte or quality oracle is accepted. | Partial Diagrammatic shared-endpoint traceability with complete field records, plus honest StructuredText primaries for unresolved or ambiguous identities. | Small/typical/dense sources retain geometry at 80/100/120 columns; the independent route oracle checks both shared targets and all typed relation markers. Record rows add vertical cost. |
| Pie | Not part of the pinned ASCII oracle or inspected moving renderer. | No Pie byte or quality oracle is accepted. | Partial Diagrammatic percentage bars share one scale and retain all sections, complete keyed labels, exact values, and metadata. Invalid distributions report StructuredText. | Small/typical/dense sources retain proportions at 80/100/120 columns; independent filled-cell checks bound rounding error. This intentionally substitutes bars for circular geometry and keeps tiny categories that Mermaid SVG filters. Trident label/completion replay is separate from proportional fidelity. |
| C4 | Neither copied nor inspected moving source has a C4 renderer; no byte or quality oracle is accepted. | No C4 byte or quality oracle is accepted. | Partial Diagrammatic Context, Container, Component, Dynamic, and Deployment views retain ordered rows, roles, nested frames, alias identity, arrow ownership, and complete keyed records. Non-spatial detached models report StructuredText. | Pinned Mermaid DB/renderer semantics and local C4 contracts own the comparison. Overview headings omit repeated details; browser assets remain records. Dense Strict routing remains bounded under the existing resource policy. See [C4 support](C4_SUPPORT.md). |
| Architecture | No Architecture byte or quality oracle is accepted. | No Architecture byte or quality oracle is accepted. | Partial Diagrammatic services, junctions, and nested groups preserve independent endpoint sides and arrows, service-anchored group contacts, ordered alignment hints, and complete records. Invalid or conflicting detached geometry reports StructuredText. | Pinned Mermaid semantics and family-local contracts own admission. Canonical/Strict, both width profiles, standard primary encodings, and Plain width fallback do not promise arbitrary dense-scene or viewport coverage. See [Architecture support](ARCHITECTURE_SUPPORT.md). |
| Railroad / Ishikawa / Quadrant | Not part of the pinned ASCII oracle. | No reference output is accepted as Mermaid byte authority for these families. | Typed core models exist, but ASCII dispatch and exhaustive capability metadata remain explicitly Unsupported. | The tracked R34 report retains source-backed test-private spatial prototypes at 80/100/120 columns: each family has a concrete dense-case failure, while Ishikawa typical and Quadrant small/typical retain positive design signals. A future complete spatial vertical slice may reopen admission. |

## Intentional Differences

- True `RL` inversion is intentional. Treating `RL` as `LR` is a reference-implementation shortcut, not
  a product goal.
- Cyclic class and ER shapes should keep rendering through the layered planner when it can produce a
  readable route, and otherwise retain every relationship in a structured summary.
- Directed Class/ER ranks follow the condensation DAG; private channels keep cyclic returns and
  their labels separate from other edges. Rings and diamonds no longer require a topology-specific
  exception, while dense or colliding scenes retain the Summary boundary.
- Wide-cell handling must treat terminal continuation cells as shared ownership, not as independent
  characters.
- Sequence `rect` and box colors should stay bounded by what the typed model and terminal can render
  without inventing browser-only semantics.

## Remaining Pressure

- CJK and emoji placement is covered for the current renderer families; keep the same semantic
  gate for new families and more complex grapheme clusters.
- Flowchart dense compound scenes beyond the bounded channel-search and congestion-repair budgets.
  The shared planner validates complete label ownership and route connectivity, but a search miss
  remains an explicit strict diagnostic rather than a claim that no presentation is possible.
  Explicit adaptive Plain mode can select a complete graph document after an admitted planning miss;
  this does not promise continuous artwork or suppress fatal operation errors.
- Class and ER dense relation topologies beyond the admitted directed rings and DAG diamonds; new policy
  decisions should keep explicit crossing, port-fit, route, and overlay boundaries. Resource budgets
  remain typed hard errors rather than summary selectors.
- XYChart dense-layout policy beyond the shipped compact plot and `values:` disclosure rows.
- C4 dense fixed-row routing and browser presentation beyond role-labelled rectangles and nested
  frames. Complete records retain details; Strict routing, resource, and cancellation failures
  remain errors. The [C4 support note](C4_SUPPORT.md) owns this boundary without a reference-parity claim.
- Architecture dense or nested spatial coverage beyond family-local tests. Declared sides,
  service anchors, and alignment order cannot be weakened to obtain a convenient layout. Invalid
  or conflicting geometry retains complete records; routing and operation failures remain errors.
  The [Architecture support note](ARCHITECTURE_SUPPORT.md) owns the boundary without an extent or
  reference-parity guarantee.
- Packet spatial admission remains bounded to integer rows of 1..=64 bits. Coordinates outside
  the exact nonnegative integer domain, non-spatial row widths/DB bit counts, overlaps, and out-of-order fields retain complete text;
  this does not justify label-driven width distortion or converting resource errors into success.
  The [Packet admission note](../ASCII_PHASE_GATE_REPORT.md#packet-bounded-spatial-admission)
  owns the verified small/typical/dense width comparison, rather than a reference parity claim.
- Railroad, Ishikawa, and Quadrant remain breadth candidates only after a new proposal
  supplies the complete spatial and width evidence required by
  `docs/rendering/ASCII_PHASE_GATE_REPORT.md`; summary-only output does not clear that gate.
