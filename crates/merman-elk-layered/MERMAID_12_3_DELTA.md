# Mermaid 12.3 ELK engine delta

This records release deltas and repairs of older gaps in the supported option surface.
It distinguishes source inspection, executed worker observations and platform boundaries;
finite fixture evidence does not establish equivalence for every Eclipse ELK option.

## Identities and delivery boundary

- Mermaid: `mermaid@12.3.0`, `2086904e76c692331e34a45d7cc3bc30860d040e`, compared with
  `1fb7869866dd619168d840291c9c54b4f129758b`.
- Behavior oracle: the installed, published `elkjs@0.12.0` package at
  `target/mermaid-12.3/cli/node_modules/elkjs`; its package-source tag/gitHead is
  `ff5771d7165445c42c408bb8a090c8035272218c`.
- Java source comparison: Eclipse ELK `323312916048544e4e594eab3303cdb0d01e2929`, compared
  with the historical port derivation `62d5909f96fad541bc101ad52dabaece6b7eab7e`.
  The elkjs tag's `.github/workflows/ci.yml` checks out Eclipse ELK **master**. The Java tag
  is a comparison point, not an independently proven unique input of the npm package build.
  The published worker's observed behavior decides numeric expectations.
- Trusted Merman base at task entry: `993c05e82ef25792e0773f4445618aa92b9601db`, branch
  `refactor/fearless-maintenance-20261103`. Existing unrelated edits were left untouched.
- The standing workspace graph was still Mermaid 12.2.0 at task entry. Its selection identity
  was `886f6d2bb684331b93b11498933c2615f92e363f61ebd7d7ed5d20ed5db615f8`; its receipt SHA-256
  was `34578989e50c2fc2980770a4c9aa44da63a74deb02b026ba0444820ea4ea296d`.
  These are historical identities at the initial ELK work-unit boundary. The current selected
  reference graph is owned by the repository bundle and decision receipt, not this record.

## Source delta classification

Paths in this table are relative to the Java comparison checkout's `plugins/` directory.
The supplied `/tmp/merman-elk-09-12-changed-sources.txt` contains 456 entries when read in this
session. Removing block comments, comment-only lines and whitespace leaves 59 files with code
changes and 397 with only comments/format changes; unused imports and Java-only changes are
still included in those 59. This is a triage count, not a semantic proof. The direct diff was
also inspected for the additional provider sources absent from that supplied list.

| Source owner / change | Disposition in the existing Rust surface |
| --- | --- |
| `alg.layered/.../intermediate/SortByInputModelProcessor.java` | Nodes are insertion-sorted before ports and again after ports. Port sorting retains the published GWT comparison schedule. Actual comparison work is charged and failed permutations are not installed. |
| `.../preserveorder/ModelOrderNodeComparator.java` | Select the first incoming port from the immediately previous layer; account for feedback long-edge helper nodes, before/after port sorting, and stable transitive relations. Reference comparisons use an iterative continuation to retain the caller's stack/work boundary. Each relation update is admitted against the current sets, including helper returns and continuation unwinding. |
| `.../preserveorder/ModelOrderPortComparator.java` | Compare each outgoing edge's own model order, replace the historical second-port lookup, use source port occurrence for a shared incoming source, respect WEST/NORTH/SOUTH reversal and same-layer long-edge ordering. Remove the former reversed-edge tiebreak branch. |
| `.../options/LongEdgeOrderingStrategy.java` | Use Java `Integer.MIN_VALUE` for `DUMMY_NODE_UNDER`; preserve the Java integer upper extreme for `DUMMY_NODE_OVER`. |
| `.../p1cycles/{ModelOrderCycleBreaker,GroupModelOrderCalculator,GreedyModelOrderCycleBreaker}.java` | Reset separate-layer counters per source node. Greedy model-order ties use layer-constrained scores. Retain importer `MAX_MODEL_ORDER_NODES` on the root and copied components. Group IDs remain unexposed. |
| `.../graph/transform/ElkGraphImporter.java` | Nonzero node/port crossing-score influence also requests model order. Existing breadth-first compound import order is retained. |
| `.../p2layers/CoffmanGrahamLayerer.java` | Add node-ID secondary comparison for empty predecessor ties. Keep the source's unusual iterator `hasNext` branch and GWT boxed-integer semantics. |
| `.../p3order/ModelOrderBarycenterHeuristic.java` | First/last-separate nodes compare equal; equal model orders fall through to barycenter. New nondefault group IDs are outside the represented surface. |
| `.../p3order/LayerSweepCrossingMinimizer.java` | Share fractional weighted scoring between flat and compound sweeps, retain the published worker's node-influence activation condition, and resolve absent fixed-port target caches without mutation. Hierarchy scoring carries caller work control; port comparisons charge each visited reference-layer node before access. The null-best-sweep parent-port guard is already represented by Rust's optional best sweep. Median is unexposed. |
| `.../intermediate/loops/{SelfLoopEdge,routing/LabelPlacer,routing/OrthogonalSelfLoopRouter}.java` | Clear inline mode for simple one-side, corner and four-side loops, not sequenced north/south loops. Sequenced labels retain inline mode and align outward in source port order. Opposing/three-side loops retain inline labels, choose the appropriate full side, omit edge-label clearance and shift adjacent corners by half the label size. |
| `core/.../util/BoxLayoutProvider.java` | Include top/bottom and then right padding in row-width area estimation. Preserve source ordering, expansion and content translation. This also changes Rectpacking's explicit `trybox` branch. |
| `alg.mrtree/.../ComponentsProcessor.java` | Recompute merged graph minima after component placement, then translate by graph left/top padding. Delete the historical first-child-padding API and last-component-minimum workaround from the adapter and kernel. |
| `alg.rectpacking/.../p2packing/InitialPlacement.java` | A new block subrow must fit within `row.y + row.height`; remove the old single-block bypass for this branch. |
| `alg.rectpacking/.../intermediate/MinSizePreProcessor.java` | Apply the single inside-top-center title grid floor only after selecting the regular packing pipeline. The earlier `trybox` return retains the raw effective minimum. A tall title with one child yields height 210; three equal children select Box and yield height 175. |
| `alg.common/.../nodespacing/internal/{NodeContext,algorithm/NodeSizeCalculator}.java` | New behavior is conditional on `TOPDOWN_LAYOUT`; that option is not represented or emitted. The supported bottom-up micro-layout behavior is unchanged, and title lifecycle observations are retained. |
| Layered compound pre/postprocessing, components/SimpleRow, normal P4/P5 algorithms; Force, Stress, Radial, SPOrE | No semantic algorithm-body change found in the compared supported paths. This does not certify the old port: the follow-up audit fixes double-precision packing, compound default thickness, empty-label import, outside labels, independent scope direction and SPOrE title margins below. Copyright/secondary-license text, unused imports and debug changes do not justify algorithm rewrites. |
| `.../graph/transform/ElkGraphLayoutTransferrer.java`, `LPadding.java`, `core/.../math/KVectorChain.java` | New nonmutating debugging export and copying helpers; no production output change to port. Rust already owns value copies. |
| `Core.melk` JSON shape/edge coordinates, label softwrapping, top-down approximators | New options are not part of the measured Rust graph API or Mermaid's emitted presets. No new JSON coordinate contract is claimed. |
| Java classloader metadata, removed imports and formatting | No headless runtime behavior to translate. Existing EPL-2.0 selection remains; the new upstream secondary-license alternative does not require changing this crate's chosen license. |

The final workspace run exposed an overestimate in relation-update admission: a single growing
side was charged as though both sides crossed every relation. The bound now follows the two
actual sets: their initial clones, direct inserts, and each side's clone/extend of the other set.
It remains linear for a one-sided update and retains the product term for crossing closures.
Feedback helper comparisons can temporarily install a reverse relation; those updates and
self-updates retain the original conservative bound because their clone sources can grow.
Admission still precedes mutation, arithmetic remains checked, and continuation-unwind rejection
stays covered. The unchanged 500-table native-font ER regression failed at `14,204,116` requested
units under the existing `14,100,000` default and passes after this correction. Neither the
default ceiling nor its historical calibration is changed; this is not a latency measurement.

## Explicitly unclosed surfaces

- New SCC/BFS/DFS cycle-breaking modes, Median crossing minimization, layer unzipping,
  group model order, optional repeated node placement, Rectpacking `orderBySize`, top-down
  layout, and VertiFlex are not represented by this API or emitted by Mermaid's admitted
  presets. They are not silently selected or advertised as supported.
- Optional admitted options that require unported processors remain typed failures, including
  actual north/south port dummy processing, interactive node placement and polyline/spline
  routing. Horizontal graph compaction, partitioning and node promotion are not exposed by
  the public options; an internal processor enum is not evidence of public reachability.
  Broader JSON coordinates and those unexposed algorithms are not closed by this work.
- Renderer `nestedEdgeOrder.ts` semantics and all family SVG/baseline changes belong to the
  coordinator. No renderer adapter, historical SVG identity, normalization rule or npm graph
  is changed here.
- The exact npm-to-Java-build mapping is not proven by the release tag. A complete upgrade
  admission still needs the coordinator's installed-byte/provenance graph, the full Rust suites,
  renderer family matrix and target checks.
- Latest-compatible elkjs 0.12.1 is separately scoped and **not fully admitted**. Its fetched
  tag is `624ae90aa37110033a8af59c5ccbb292708b4317`; `build.gradle` selects
  `melk=0.13.0-SNAPSHOT`, GWT 2.13 and DisCo sources. These are real build/behavior deltas, not
  a fabricated rejection or evidence that 0.12.1 equals 0.12.0. Outside-range 0.13 remains a
  separate evaluation. The materialized 0.12.0 checkout was not moved.

## Executed runtime evidence

The optional package-owned collector is `tests/elkjs_0_12_0_oracle.cjs`. It refuses any package
version other than 0.12.0 and runs without installation or lifecycle scripts:

```text
node crates/merman-elk-layered/tests/elkjs_0_12_0_oracle.cjs \
  /Users/frankorz/Documents/projects/rust/merman/target/mermaid-12.3/cli/node_modules/elkjs \
  target/mermaid-12.3/elk-engine/oracle-0.12.0-randomized-tries.json
```

The JSON retains complete inputs and raw outputs, collection time, Node identity and hashes:

- Public minified worker SHA-256:
  `e4f5c51c6f2fd564bb64e52e81c4a85a316ef2f24f8b90e1db5eb886c765b030`.
- Pretty worker used for phase observation SHA-256:
  `f7617e622f565748e1c88a9f45d97b458bc9c3f4299c0fdc6ed778a92ef30683`.
- The initial 82 input cases cover all supported providers, four directions, packing minima/padding,
  components, cycles, source-model ports, inline self-loops, compound edges, explicit containers,
  mixed provider scheduling and title micro-layout lifecycle.
- Every case is executed twice through the unmodified public package and once through an
  observer that wraps phase calls without replacing algorithm bodies. Public geometry is
  identical across the three runs. Only GWT's `$H` object identity field is excluded from
  that replay assertion; the complete first public output and all geometry remain available.
- The observer captures Coffman-Graham layer arrays and `SortByInputModelProcessor` node/port
  arrays. Source-derived independent Rust tests cover layer constraints and importer decisions.

The initial 82-case evidence above remains at `oracle-0.12.0.json` (SHA-256
`3da1e6605609c04dec5dff66e860f50f4c78c6b0b23b454725d3d71f79167f3b`). The collector now also
includes the historical compound-cycle randomized-tries input. Its expanded 83-case collection
is `target/mermaid-12.3/elk-engine/oracle-0.12.0-randomized-tries.json` (SHA-256
`b3343903eaaa4fa78374d0616b80fa228adbabbdb493f877fd7c54bc13a4d3f6`, collected
`2026-11-03T16:57:32.896Z`), with 24 phase/attempt observations. The command above replays
the expanded collection. Both package runs and the observed worker still agree.

For `hierarchical-randomized-tries`, the published 0.12.0 worker reports root crossing counts
`[2, 2, 2, 1, 2, 1, 2]`. Its first strictly best run installs `[B, bar, L_D_E dummy]` in the
first layer and `[foo, L_D_E dummy]` in the last layer. The historical 0.9.3 assertion was
`[L_D_E dummy, bar, B]` / `[L_D_E dummy, foo]`. The source still gives FIRST/SECOND_TRY the
same Property ID. The existing Rust regression now checks the target crossing sequence and
initial-order flag as well as the observed layer order; the historical comment stays historical.

Historical 0.9.3 values, worker identities and derivation links stay historical. Where behavior
changed, tests retain the old measurements and append target expectations. Mr. Tree's retained
shape/route corpus compares in the new, separately measured coordinate frame; a new test also
checks target coordinates and routes directly. No comparison tolerance is widened.

One historical explicit wide-title Stress expectation (`width=330`) did not reproduce for the
collector's matching input. Actual 0.9.3 and 0.12.0 both return `179.74360154767774`, because
Stress's Force initialization fixes the graph size before the parent Box invocation. The current
adapter already models this lifecycle. Its test retains the historical value and appends the
target observation; this correction is not attributed to a version change.

## Follow-up audit: old gaps and source/runtime comparison

The independent review at `d8b904987aad6baa95ce917653794323d338799e` rechecked the
Mermaid 12.2-to-12.3 and ELK derivation-to-0.12 source diffs, including P1–P5,
compound import, node spacing, components and the provider adapter. Normal P4/P5
algorithm bodies do not change in the compared sources; that conclusion is not
used to excuse pre-existing port defects. The resulting repairs also cover:

- Empty strings are omitted when constructing node and edge `LLabel`s; whitespace
  is retained. Raw owner-label geometry still contributes to compound padding and
  minimum calculation **before** this text filter, following the worker's import
  order. Measured adapter labels without a text field use a nonempty U+FFFC
  placeholder rather than masquerading as authored empty strings.
- Ordinary and cross-hierarchy edge segments share the default thickness of 1.
  A 10-high non-inline center label with spacing 2 needs a 14-high dummy, not 12.
  Restored hierarchical-port helper edges also retain this default.
- All twelve public outside-node-label placements position whole label groups,
  preserve direction transforms and reserve their actual margins. Fixed labels
  retain their coordinates; inside labels keep their existing behavior.
- Component packing retains JavaScript binary64 arithmetic. Runtime `as f32`
  narrowing is removed from Layered, Force and Mr. Tree. Box's default aspect
  ratio retains the selected worker's widened float literal; an explicit ratio
  and its nonpositive fallback remain separate contracts.
- Separately scheduled Layered containers resolve their own default direction;
  compound descendants still inherit within their shared invocation. Flowchart
  groups use FlowDB's fixed padding 8, not configurable leaf padding.
- A separately scheduled Radial title keeps its imported origin and contributes
  its overhanging margin to the parent SPOrE importer. No fake edge route or
  pixel-tuning constant is added.

The maintained collector now includes tall-title regular/Box branches, fixed-port
and fractional scoring, sequenced loops, precision thresholds, all twelve outside
positions in four directions and empty/whitespace label imports. It records full
public inputs/outputs, reused-instance replay and nonmutating phase observations.
The exact command above regenerates this ignored evidence; the selected worker
hashes remain unchanged. The follow-up completion receipt records final counts
and verification, rather than treating these observations as a new reference lock.

A same-input replay of 227 follow-up inputs through the retained published
0.9.3 and selected 0.12.0 packages found 180 identical geometric projections and
47 changed ones. This distinguishes actual release deltas from old port gaps:
empty-label filtering, outside placement and default thickness were already
required, while the observed sequenced-inline and tall-Rectpacking cases change
across the two workers. Equal outputs for these finite inputs do not prove
complete version equivalence.

## Initial coordinator verification

Coordinator follow-up on 2026-11-04: the corrected two-crate suite passed all 513 tests with
no skips (`/tmp/merman-12.3-elk-all-green.log`). Both crate doctest owners passed, including
the adapter's compile-fail boundary, and all-target Clippy passed with `-D warnings` after
correcting two documentation paragraph separators. These results validate this Rust unit;
renderer family comparisons and complete reference admission remain separate gates below.

The follow-up two-crate suite passes 583 tests without skips, including raw-title import
order, outside positions, scoring, work interruption, numeric thresholds and sequenced loop
coordinates by edge identity. Renderer tests additionally check fixed group padding and
independent-scope direction. The repository upgrade playbook owns the integrated verification
schedule; completed worker instructions and duplicate command lists are retired here.

No new dependency or feature boundary is introduced. All algorithm translations stay in the
existing EPL-2.0 crate; the MIT/Apache adapter delegates rather than translating algorithms.
