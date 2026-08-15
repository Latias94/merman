# ASCII Phase 0-3 Gate and New-Family Admission Report

Status: Phase 0-3 evidence complete; Phase 4 candidates not admitted
Baseline: pinned Mermaid `11.16.1`
Last updated: 2026-08-14

This report is the tracked gate artifact required by U16 and U17 of the ASCII semantic-depth plan.
It records why common-family depth remains the priority and why Railroad, Requirement, Ishikawa,
and Quadrant remain Unsupported. It does not promote any existing Partial family to Full.

The report distinguishes two decisions:

1. Phase 0-3 evidence is sufficient to evaluate new-family proposals without weakening the shared
   terminal contract.
2. Test-private spatial prototypes for all four proposals fail at least one required recovery or
   information-gain case, so no Phase 4 implementation or public capability was admitted.

The second decision is a conservative release disposition, not a claim that these families can
never be useful in a terminal. A future proposal can reopen one family by supplying the complete
spatial vertical slice and executable width evidence described below.

## Authority and Reproduction

The evidence boundary is:

- Mermaid `11.16.1`, pinned by `tools/upstreams/REPOS.lock.json`, for syntax and typed semantics;
- `mermaid-ascii` commit `6fffb8e2714acab2c4cb41c78894fabbc62cee56` for the immutable copied
  graph and sequence byte oracle;
- local moving-reference inspection at
  `b1b35f67d6a5dd0699ccfc968c00a763db573076`, classified in
  `crates/merman-ascii/ASCII_MOVING_REFERENCE_MANIFEST.md` rather than used as a release oracle;
- `beautiful-mermaid` commit `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488` for capability prior art
  only; and
- tracked local parser/model, semantic, resource, capability, and fixture-inventory tests for the
  product contract.

Release CI does not read `repo-ref/`. The optional checkouts are discovery inputs; every release
claim below points to tracked source, fixtures, tests, or documentation.

The authoritative reproduction commands are:

```text
cargo nextest run -p merman-ascii -j 1
cargo nextest run -p merman-core -j 1
cargo nextest run -p merman-render -j 1
cargo nextest run -p merman --features ascii -j 1
cargo nextest run -p merman-bindings-core --features ascii -j 1
cargo nextest run -p merman-uniffi --features ascii -j 1
cargo nextest run -p merman-wasm --features ascii -j 1
cargo nextest run -p merman-cli --no-default-features --features ascii -j 1
```

The implementation-package commands were observed during the phased work. This report does not
contain an exact-current-HEAD receipt, so closeout remains provisional until a tracked receipt binds
the final source revision, commands, environment, and results. Prose and untracked local drafts are
not substitutes for that evidence.

## Phase 0-3 Gate

| Phase | Evidence result | Tracked evidence | Admission consequence |
| --- | --- | --- | --- |
| Phase 0: terminal contract | Complete | `terminal_text_safety.rs`, `resource.rs`, `canvas.rs`, binding resource tests, and exhaustive capability tests cover safe text, grapheme ownership, checked extent, document/output budgets, and all 31 typed families. | A new family must consume the same safe-text, width-profile, resource, and encoder contracts; it cannot add an unbudgeted private renderer. |
| Phase 1: Flowchart and Sequence | Complete at Partial | The immutable graph corpus is 40/79 exact plus 39 named renderable differences. Sequence keeps 17/17 normalized copied parity. Its 322-case imported corpus contains 321 Mermaid-valid fixtures that parse and render plus one intentional-invalid stress fixture that is rejected; promoted local semantic probes own broader marker, compound, signal, control, lifecycle, and actor claims. | Common families remain Partial where documented residuals exist; their evidence is not weakened to make a new family easier to add. |
| Phase 2: Class, ER, State, XYChart | Complete at Partial | Class/ER imported fixtures prove admission, while shared relation-component probes, State model tests, and XYChart typed-coordinate tests own the retained semantic boundary and explicit fallbacks. | A new family may reuse mechanisms such as relation planning or plot cells, but must keep family semantics in its own adapter. |
| Phase 3: existing-family truth | Complete | Runtime capabilities classify six families as Diagrammatic and eight as StructuredText. The support matrix records the retained fields and limits for all 14 available outputs. | StructuredText remains useful output but contributes zero to the ASCII diagram count and cannot satisfy a new-family admission. |

The moving-reference lane contains 140 uniquely identified paths with validity, admission, semantic
feature, and evidence dispositions. The copied oracle remains immutable. The complete comparison
policy and exact counts live in `crates/merman-ascii/ASCII_REFERENCE_COMPARISON.md` and
`crates/merman-ascii/V1_MERMAID_ASCII_COVERAGE.md`.

## Existing StructuredText Dispositions

The Phase 3 decision is to retain Gantt, GitGraph, Journey, Kanban, Mindmap, Packet, Timeline, and
TreeView as StructuredText. Their reports preserve typed facts useful in logs and review, but none
claims timeline, lane, board, map, packet-width, spine, or two-dimensional tree geometry. This is a
binding R34 rejection of a Diagrammatic promotion, not a degradation fallback selected by resource
pressure.

| Family | Representative terminal task | Diagrammatic decision |
| --- | --- | --- |
| Gantt | Inspect task identity, constraints, sections, flags, and resolved times. | Retain StructuredText; no aligned date-scale timeline was admitted. |
| GitGraph | Trace commit order, branches, merges, tags, and parent identities. | Retain StructuredText; no lane graph was admitted. |
| Journey | Review ordered tasks, actors, sections, and scores. | Retain StructuredText intentionally; a browser-shaped journey adds insufficient terminal value. |
| Kanban | Audit column/card identity, assignment, and metadata, including orphan handling. | Retain StructuredText; no spatial board was admitted. |
| Mindmap | Recover hierarchy, identity, cycles, and disconnected components. | Retain StructuredText; the outline is useful, but it is not counted as two-dimensional geometry. |
| Packet | Audit exact ordered bit ranges and labels. | Retain StructuredText; character width does not encode bit width. |
| Timeline | Inspect direction, sections, and ordered events. | Retain StructuredText; no spatial time spine was admitted. |
| TreeView | Inspect file/directory hierarchy and disclosed node metadata. | Retain StructuredText; charset-aware tree prefixes are an outline, not a two-dimensional diagram. |

## U30 Structural Closeout Disposition

The changed integration tests are now partitioned by semantic owner. Flowchart and Sequence keep
private parent modules for shared parser/options helpers, while the former multi-family
`new_family_models` target delegates to TreeView, Mindmap, Timeline, Gantt, Journey, Kanban,
Packet, and GitGraph modules. The split preserves one executable target and one shared harness
without leaving unrelated family behavior in the same source file.

The remaining large Sequence roots are retained as pipeline coordinators rather than mechanism
owners. Lexical scanning, typed events, lifecycle validation, control geometry and paint, notes,
boxes, prepared bodies, text, and recursive control trees already have separate modules.
`sequence/model.rs` owns the single typed-model projection transaction, and `sequence/plan.rs`
owns the admission-ordered body/control/document pipeline. Splitting either coordinator again
would expose partially admitted row state or create pass-through modules. A future independent
algorithm must be extracted behind a replayable plan or paint descriptor instead of being added
as another branch in either root.

`relation_graph.rs` is retained as the component and document coordinator. Family semantics remain
in Class and ER adapters; document planning, horizontal routing, layered placement and single-route
geometry, self loops, stacking, summaries, and the shared model are separate owners. Layered batch
planning now has its own deep module: it owns scene admission, the resource transaction, route and
overlay materialization, collision validation, and pairwise work accounting. Its interface returns
only a paint plan or a typed summary reason, while preserving the rule that semantic fallback keeps
speculative layout work and resource failure rolls back the transaction. The root now selects
component strategies, maps that outcome to a region or family-owned summary, aggregates regions,
and performs the final deferred document commit. New geometry belongs in the corresponding layered
module, new encoding belongs in the document/canvas path, and any new fallback policy with an
independent lifecycle must first be extracted behind a typed outcome.

These are cohesion boundaries, not line-count exemptions. Repository review must reopen U30 if a
second independent transaction, encoding lifecycle, or fallback policy is added to either retained
root.

## New-Family Admission Method

R34 requires positive evidence. For each candidate, an evaluation must name a user task, small,
typical, and dense source fixtures, the spatial fact a reader should recover, the information gain
over a field-complete StructuredText baseline, and the narrow/dense failure policy. Views at 80,
100, and 120 columns must exist and remain scannable.

The executable evaluator in `crates/merman-ascii/tests/candidate_admission/evaluator.rs` consumes the
real typed models for the 12 fixtures below. It creates bounded 7-bit prototypes solely for this
gate; it is not linked from the production dispatcher. Width is an explicit evaluator input, so
the 80/100/120 results do not infer terminal behavior from `UnsupportedDiagram`.

The comparison is deliberately fact-based:

- Railroad counts typed choice, optional, and repetition operators and asks whether the candidate
  represents them as connected rail topology rather than only as words.
- Requirement counts typed relations and separately rejects repeated node placements that turn a
  convergence into independent tree copies.
- Ishikawa counts every typed parent-child edge and asks whether each edge is connected to the
  effect/spine projection.
- Quadrant counts distinct typed point positions after width-dependent quantization; the exact
  point table is the lossless StructuredText baseline, not credit for spatial recovery.

For every matrix row the StructuredText baseline recovers all facts in the denominator. "Gain"
is Yes only when the prototype also recovers the topology, the spatial pattern is non-trivial, and
the view is more scannable than that baseline. A positive small or typical result is retained as
design evidence, but it cannot compensate for a failed required semantic or dense case. All 36
prototype views fit their stated width without clipping.

<a id="railroad-r34"></a>
## Railroad R34 Disposition

- Representative user task: inspect grammar sequence, alternatives, optionals, and repetition
  cardinality without opening a browser.
- Small fixture: `fixtures/railroad/upstream_cypress_railroad_spec_renders_a_simple_rule_001.mmd`.
- Typical fixture:
  `fixtures/railroad/upstream_cypress_railroad_spec_renders_sequences_and_choices_002.mmd`.
- Dense fixture:
  `fixtures/railroad/upstream_cypress_railroad_spec_renders_multiple_rules_in_one_diagram_004.mmd`.
- Spatial fact required for admission: every entry-to-exit path must preserve ordered sequence,
  branch alternatives, optional bypasses, repetition bounds, and separators.
- StructuredText baseline: a recursive grammar report can disclose the typed AST, but it does not
  make alternative paths or loop cardinality more scannable and therefore receives no spatial
  credit.
- Candidate prototype: expand alternatives into entry-to-exit rows. This makes choices visible,
  but repetition is still a textual `loop[min..max]` token rather than a connected back edge. The
  prototype treats at most 12 expanded rows as scannable; the dense grammar expands to 24 rows and
  repeats common prefixes at every width. The 12-row threshold is an explicit candidate policy,
  not a claim about all future rail layouts.

| Case | Width | Spatial facts | Topology recoverable | Gain over StructuredText | Observation |
| --- | ---: | ---: | --- | --- | --- |
| Small | 80 | 0/0 | Yes | No | one linear route adds no scan advantage over the grammar record |
| Small | 100 | 0/0 | Yes | No | one linear route adds no scan advantage over the grammar record |
| Small | 120 | 0/0 | Yes | No | one linear route adds no scan advantage over the grammar record |
| Typical | 80 | 3/5 | No | No | 3/5 choice/optional/repetition operators are spatial; repetition remains a text token |
| Typical | 100 | 3/5 | No | No | 3/5 choice/optional/repetition operators are spatial; repetition remains a text token |
| Typical | 120 | 3/5 | No | No | 3/5 choice/optional/repetition operators are spatial; repetition remains a text token |
| Dense | 80 | 4/7 | No | No | 4/7 choice/optional/repetition operators are spatial; repetition remains a text token; 24 route rows also duplicate shared prefixes |
| Dense | 100 | 4/7 | No | No | 4/7 choice/optional/repetition operators are spatial; repetition remains a text token; 24 route rows also duplicate shared prefixes |
| Dense | 120 | 4/7 | No | No | 4/7 choice/optional/repetition operators are spatial; repetition remains a text token; 24 route rows also duplicate shared prefixes |

The dense output is byte-identical at 80, 100, and 120 columns because its longest row already
fits at 80; extra width does not repair the missing loop geometry or the 24-row expansion:

```text
json:
  `--o--<element>--o
element:
  +--o--<object>--o
  +--o--<array>--o
  +--o--<string>--o
  +--o--<number>--o
  +--o--[true]--o
  +--o--[false]--o
  `--o--[null]--o
object:
  +--o--[{]--bypass--[}]--o
  `--o--[{]--<member>--loop[0..*]([,]--<member>)--[}]--o
array:
  +--o--[[]--bypass--[]]--o
  `--o--[[]--<element>--loop[0..*]([,]--<element>)--[]]--o
member:
  `--o--<string>--[:]--<element>--o
number:
  `--o--loop[1..*](<digit>)--o
digit:
  +--o--[0]--o
  +--o--[1]--o
  +--o--[2]--o
  +--o--[3]--o
  +--o--[4]--o
  +--o--[5]--o
  +--o--[6]--o
  +--o--[7]--o
  +--o--[8]--o
  `--o--[9]--o
```

Disposition: **Unsupported**. The prototype demonstrates some choice-path value, but it fails the
typed repetition topology and dense scannability requirements. Reopen with connected loop rails
and a layout that shares prefixes instead of route enumeration.

<a id="requirement-r34"></a>
## Requirement R34 Disposition

- Representative user task: trace which elements satisfy, verify, refine, derive, contain, copy, or
  trace requirements while inspecting requirement id, text, risk, and verification method.
- Small fixture: `fixtures/requirement/basic.mmd`.
- Typical fixture: `fixtures/requirement/relations.mmd`.
- Dense fixture: `fixtures/requirement/upstream_docs_requirementdiagram_larger_example_010.mmd`.
- Spatial fact required for admission: each typed relation kind and direction must remain attached
  to the correct source and target while node fields remain inspectable exactly once.
- StructuredText baseline: requirement records plus a typed edge list can be lossless, but a list is
  not evidence that traceability is easier than reading those records directly.
- Candidate prototype: expand outgoing relations as connected trees. The one-edge view is exact but
  no easier than the edge record. The dense fixture draws all eight relation facts once, but
  `test_req2` and `test_req5` appear in separate branches instead of showing the two convergences.

| Case | Width | Spatial facts | Topology recoverable | Gain over StructuredText | Observation |
| --- | ---: | ---: | --- | --- | --- |
| Small | 80 | 0/0 | Yes | No | 0 relation adds no scan advantage over the typed edge record |
| Small | 100 | 0/0 | Yes | No | 0 relation adds no scan advantage over the typed edge record |
| Small | 120 | 0/0 | Yes | No | 0 relation adds no scan advantage over the typed edge record |
| Typical | 80 | 1/1 | Yes | No | 1 relation adds no scan advantage over the typed edge record |
| Typical | 100 | 1/1 | Yes | No | 1 relation adds no scan advantage over the typed edge record |
| Typical | 120 | 1/1 | Yes | No | 1 relation adds no scan advantage over the typed edge record |
| Dense | 80 | 8/8 | No | No | 8/8 edges are present, but 2 repeated placements (test_req2, test_req5) hide global node identity |
| Dense | 100 | 8/8 | No | No | 8/8 edges are present, but 2 repeated placements (test_req2, test_req5) hide global node identity |
| Dense | 120 | 8/8 | No | No | 8/8 edges are present, but 2 repeated placements (test_req2, test_req5) hide global node identity |

The dense output is also identical at all three widths; width cannot turn copied tree nodes into
shared relation endpoints:

```text
[test_entity]
`-- satisfies --> [test_req2]
[test_entity2]
`-- copies --> [test_req]
    +-- traces --> [test_req2] (repeat)
    `-- contains --> [test_req3]
        `-- contains --> [test_req4]
            `-- derives --> [test_req5]
                `-- refines --> [test_req6]
[test_entity3]
`-- verifies --> [test_req5] (repeat)
```

Disposition: **Unsupported**. The prototype preserves relation records but does not provide a
global node-owned graph, so it adds no traceability advantage over the lossless edge list. Reopen
with shared endpoints, direction-aware layout, and Requirement-owned relation labels/markers.

<a id="ishikawa-r34"></a>
## Ishikawa R34 Disposition

- Representative user task: follow an observed effect through major causes into nested root causes.
- Small fixture:
  `fixtures/ishikawa/upstream_cypress_ishikawa_spec_4_should_render_with_a_single_cause_004.mmd`.
- Typical fixture:
  `fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd`.
- Dense fixture:
  `fixtures/ishikawa/upstream_cypress_ishikawa_spec_3_should_render_with_deeply_nested_causes_003.mmd`.
- Spatial fact required for admission: the effect/spine, each parent-child edge, sibling ownership,
  and source order must remain recoverable as one connected projection.
- StructuredText baseline: an indented outline can preserve the tree, but the plan explicitly does
  not treat indentation alone as a fishbone diagram or as evidence of additional spatial value.
- Candidate prototype: alternate first-level causes around an effect spine and attach direct
  children to each branch label. This is useful for the typical two-level case, but the recursive
  dense fixture has four deeper edges that never receive a connector.

| Case | Width | Spatial facts | Topology recoverable | Gain over StructuredText | Observation |
| --- | ---: | ---: | --- | --- | --- |
| Small | 80 | 1/1 | Yes | No | one cause/effect edge adds no scan advantage over the two-line outline |
| Small | 100 | 1/1 | Yes | No | one cause/effect edge adds no scan advantage over the two-line outline |
| Small | 120 | 1/1 | Yes | No | one cause/effect edge adds no scan advantage over the two-line outline |
| Typical | 80 | 4/4 | Yes | Yes | all 4 cause edges are connected around the effect spine |
| Typical | 100 | 4/4 | Yes | Yes | all 4 cause edges are connected around the effect spine |
| Typical | 120 | 4/4 | Yes | Yes | all 4 cause edges are connected around the effect spine |
| Dense | 80 | 5/9 | No | No | 5/9 parent-child edges are connected; descendants below depth two lose ownership |
| Dense | 100 | 5/9 | No | No | 5/9 parent-child edges are connected; descendants below depth two lose ownership |
| Dense | 120 | 5/9 | No | No | 5/9 parent-child edges are connected; descendants below depth two lose ownership |

The 80-column dense view uses a 61-cell spine; the 100- and 120-column views both use the bounded
64-cell spine. Their only difference is three extra spine cells, so all three lose the same four
deep edges:

```text
                                      Disk, Memory / Hardware
                                                            \
==============================================================> [Server Outage]
                                                            /
                                               Bug \ Software
[prototype omits 4 deeper edge(s)]
```

```text
                                         Disk, Memory / Hardware
                                                               \
=================================================================> [Server Outage]
                                                               /
                                                  Bug \ Software
[prototype omits 4 deeper edge(s)]
```

Disposition: **Unsupported**. The positive typical result confirms that fishbone geometry can be
useful in a terminal, but the candidate fails recursive ownership on the source-backed dense case.
Reopen with a depth-bounded connected-tree plan; an appended outline cannot supply the missing
spatial edges.

<a id="quadrant-r34"></a>
## Quadrant R34 Disposition

- Representative user task: compare products or campaigns by relative x/y position and recover
  exact quadrant membership.
- Small fixture: `fixtures/quadrantchart/stress_quadrantchart_batch1_boundaries_001.mmd`.
- Typical fixture:
  `fixtures/quadrantchart/upstream_cypress_quadrantchart_spec_should_render_a_complete_quadrant_chart_002.mmd`.
- Dense fixture:
  `fixtures/quadrantchart/stress_quadrantchart_batch1_dense_points_overlap_003.mmd`.
- Spatial fact required for admission: axes, quadrant ownership, relative x/y order, boundaries, and
  colliding point identities must remain recoverable; exact coordinates must also be disclosed.
- StructuredText baseline: an exact point table preserves coordinates and quadrant assignment, but
  without a grid it provides no visual position advantage.
- Candidate prototype: reserve the disclosure area and quantize points into a
  `viewport_width / 2 - 12` grid (28, 38, or 48 cells at the required widths), then append an exact
  point table. Boundaries and the typical campaign distribution remain distinct. In the dense fixture,
  the nine typed positions occupy only two cells at every width because all three y values quantize
  to the same row; the grid cannot recover their relative y order.

| Case | Width | Spatial facts | Topology recoverable | Gain over StructuredText | Observation |
| --- | ---: | ---: | --- | --- | --- |
| Small | 80 | 5/5 | Yes | Yes | all 5 point positions and quadrants remain spatially distinct |
| Small | 100 | 5/5 | Yes | Yes | all 5 point positions and quadrants remain spatially distinct |
| Small | 120 | 5/5 | Yes | Yes | all 5 point positions and quadrants remain spatially distinct |
| Typical | 80 | 6/6 | Yes | Yes | all 6 point positions and quadrants remain spatially distinct |
| Typical | 100 | 6/6 | Yes | Yes | all 6 point positions and quadrants remain spatially distinct |
| Typical | 120 | 6/6 | Yes | Yes | all 6 point positions and quadrants remain spatially distinct |
| Dense | 80 | 2/9 | No | No | 2/9 distinct grid positions remain; 9 points collide and their relative order is recoverable only from the exact table |
| Dense | 100 | 2/9 | No | No | 2/9 distinct grid positions remain; 9 points collide and their relative order is recoverable only from the exact table |
| Dense | 120 | 2/9 | No | No | 2/9 distinct grid positions remain; 9 points collide and their relative order is recoverable only from the exact table |

The dense plot rows below are the actual 80-, 100-, and 120-column evaluator views. `*` means
multiple point identities share one terminal cell. The exact table is identical in all three views
and is shown once after the grids:

```text
80 columns / 28 plot cells
+----------------------------+
|             |              |
|             |              |
|             |              |
|             |              |
|-------------+**------------|
|             |              |
|             |              |
|             |              |
|             |              |
+----------------------------+
```

```text
100 columns / 38 plot cells
+--------------------------------------+
|                  |                   |
|                  |                   |
|                  |                   |
|                  |                   |
|------------------+**-----------------|
|                  |                   |
|                  |                   |
|                  |                   |
|                  |                   |
+--------------------------------------+
```

```text
120 columns / 48 plot cells
+------------------------------------------------+
|                       |                        |
|                       |                        |
|                       |                        |
|                       |                        |
|-----------------------+**----------------------|
|                       |                        |
|                       |                        |
|                       |                        |
|                       |                        |
+------------------------------------------------+
```

```text
points (exact):
1 A-09 x=0.540 y=0.540 Q1
2 A-08 x=0.530 y=0.540 Q1
3 A-07 x=0.520 y=0.540 Q1
4 A-06 x=0.540 y=0.530 Q1
5 A-05 x=0.530 y=0.530 Q1
6 A-04 x=0.520 y=0.530 Q1
7 A-03 x=0.540 y=0.520 Q1
8 A-02 x=0.530 y=0.520 Q1
9 A-01 x=0.520 y=0.520 Q1
```

The exact disclosure lists all nine labels and coordinates, but that recovery belongs to the
StructuredText baseline. It cannot turn two occupied cells into nine spatially distinct points.

Disposition: **Unsupported**. Small and typical charts show real terminal value, but the current
candidate fails dense positional recovery at all required widths. Reopen with a collision strategy
that preserves relative order without relying on the point table for the missing spatial facts.

## Binding Decision

Railroad, Requirement, Ishikawa, and Quadrant remain present in the exhaustive 31-family runtime
capability metadata with `semantic_coverage = null`, `primary_projection = none`, and derived
`support_level = unsupported`. They remain absent from both supported-output and diagrammatic lists.
The typed dispatch returns `UnsupportedDiagram`; there is no dispatcher stub, summary-only shortcut,
binding/Web advertisement, or fixture-output artifact to mistake for admission.

The tests `crates/merman-ascii/tests/candidate_admission.rs` and its private evaluator module bind
this report to the runtime capability state, representative tracked fixtures, actual typed-model
prototype measurements, the explicit unsupported dispatch, and the four 80/100/120 rejection
matrices.
