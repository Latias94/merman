# Mermaid 12.3 ELK engine delta

This is an implementation and test handoff for the existing supported option surface. It is
not a claim that every Eclipse ELK option or every renderer fixture is aligned.

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
  Reference descriptors, npm/Cargo locks, renderer adapters and release admission stay with
  the coordinator. This unit changes only the two ELK crates, without commits or Cargo runs.

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
| `.../p3order/LayerSweepCrossingMinimizer.java` | Use the updated comparators for model-order scoring. The source's null-best-sweep parent-port guard is already represented by Rust's optional best sweep. Median is unexposed. |
| `.../intermediate/loops/{SelfLoopEdge,routing/LabelPlacer,routing/OrthogonalSelfLoopRouter}.java` | Clear unsupported inline mode on one-side, corner and four-side loops. Opposing/three-side loops retain inline labels, choose the appropriate full side, omit edge-label clearance and shift adjacent corners by half the label size. |
| `core/.../util/BoxLayoutProvider.java` | Include top/bottom and then right padding in row-width area estimation. Preserve source ordering, expansion and content translation. This also changes Rectpacking's explicit `trybox` branch. |
| `alg.mrtree/.../ComponentsProcessor.java` | Recompute merged graph minima after component placement, then translate by graph left/top padding. Delete the historical first-child-padding API and last-component-minimum workaround from the adapter and kernel. |
| `alg.rectpacking/.../p2packing/InitialPlacement.java` | A new block subrow must fit within `row.y + row.height`; remove the old single-block bypass for this branch. |
| `alg.rectpacking/.../intermediate/MinSizePreProcessor.java` | Its new label-derived minimum is already covered for the represented single inside-top-center title by the adapter's effective minimum. Wide/narrow title observations cover the production metadata preset. |
| `alg.common/.../nodespacing/internal/{NodeContext,algorithm/NodeSizeCalculator}.java` | New behavior is conditional on `TOPDOWN_LAYOUT`; that option is not represented or emitted. The supported bottom-up micro-layout behavior is unchanged, and title lifecycle observations are retained. |
| Layered compound pre/postprocessing, components/SimpleRow, normal P4/P5 algorithms; Force, Stress, Radial, SPOrE | No semantic algorithm-body change found in the compared supported paths. Copyright/secondary-license text, unused imports and debug changes do not justify algorithm rewrites. Runtime observations cover representative inputs; this is not exhaustive geometry equivalence. |
| `.../graph/transform/ElkGraphLayoutTransferrer.java`, `LPadding.java`, `core/.../math/KVectorChain.java` | New nonmutating debugging export and copying helpers; no production output change to port. Rust already owns value copies. |
| `Core.melk` JSON shape/edge coordinates, label softwrapping, top-down approximators | New options are not part of the measured Rust graph API or Mermaid's emitted presets. No new JSON coordinate contract is claimed. |
| Java classloader metadata, removed imports and formatting | No headless runtime behavior to translate. Existing EPL-2.0 selection remains; the new upstream secondary-license alternative does not require changing this crate's chosen license. |

## Explicitly unclosed surfaces

- New SCC/BFS/DFS cycle-breaking modes, Median crossing minimization, layer unzipping,
  group model order, optional repeated node placement, Rectpacking `orderBySize`, top-down
  layout, and VertiFlex are not represented by this API or emitted by Mermaid's admitted
  presets. They are not silently selected or advertised as supported.
- Pre-existing unsupported processors remain typed failures, including north/south port dummy
  processing and horizontal graph compaction. Their upstream changes (model-order port reversal,
  self-loop-label compaction offset), partition preprocessing, component-group placement and
  broader JSON coordinates are not closed by this work.
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

## Changed files

All paths below are repository-relative. The generated oracle JSON remains under ignored
`target/` and is not a standing reference descriptor.

```text
crates/merman-elk-layered/MERMAID_12_3_DELTA.md
crates/merman-elk-layered/README.md
crates/merman-elk-layered/src/algorithms/box_layout.rs
crates/merman-elk-layered/src/algorithms/mrtree/mod.rs
crates/merman-elk-layered/src/algorithms/mrtree/tests.rs
crates/merman-elk-layered/src/algorithms/rectpacking/packing.rs
crates/merman-elk-layered/src/algorithms/rectpacking/tests.rs
crates/merman-elk-layered/src/graph.rs
crates/merman-elk-layered/src/importer.rs
crates/merman-elk-layered/src/lib.rs
crates/merman-elk-layered/src/options.rs
crates/merman-elk-layered/src/p1cycles.rs
crates/merman-elk-layered/src/p2layers/coffman_graham.rs
crates/merman-elk-layered/src/p2layers/elkjs_0_9_3_cases.rs
crates/merman-elk-layered/src/p3order.rs
crates/merman-elk-layered/src/p3order/sweep.rs
crates/merman-elk-layered/src/pipeline.rs
crates/merman-elk-layered/src/selfloops.rs
crates/merman-elk-layered/tests/elk_0_12_0.rs
crates/merman-elk-layered/tests/elkjs_0_12_0_oracle.cjs
crates/merman-layout-elk/README.md
crates/merman-layout-elk/src/flat.rs
crates/merman-layout-elk/src/flat/tests.rs
crates/merman-layout-elk/src/lib.rs
```

## Coordinator verification

This unit did not run Cargo. The coordinator reported 7/7 focused tests passed and 511/512
whole-crate tests passed in `/tmp/merman-12.3-elk-all.log`. The sole failure was the historical
compound-cycle layer-order assertion described above. Its target-backed correction and stronger
attempt assertions still require the coordinator's rerun. A follow-up caller-budget regression
also covers a 256-dummy reference chain and interruption before its last unwound relation is
installed. The source helper's repeated relation updates retain their call order and each gets
its own admission check. These follow-ups require a full crate rerun after the focused checks.
Standalone rustfmt parse/format
checking and `git diff --check` were run without writing source files outside `apply_patch`.
The coordinator should run these commands serially:

```text
cargo nextest run --locked -p merman-elk-layered hierarchical_barycenter_keeps_current_order_between_randomized_tries --no-fail-fast
cargo nextest run --locked -p merman-elk-layered feedback_reference_comparisons_charge_each_unwound_relation --no-fail-fast
cargo nextest run --locked -p merman-elk-layered --test elk_0_12_0 --no-fail-fast
cargo nextest run --locked -p merman-elk-layered -p merman-layout-elk --no-fail-fast
cargo test --locked -p merman-elk-layered -p merman-layout-elk --doc
cargo fmt --all -- --check
cargo clippy --locked -p merman-elk-layered -p merman-layout-elk --all-targets -- -D warnings
```

The whole-crate suites include the existing execution-boundary tests for unresolved Java seeds,
work interruption, numeric failure and input preservation. Renderer ELK/family tests and strict
reference/DOM admission follow under the coordinator's broader scope.

No new dependency or feature boundary is introduced. All algorithm translations stay in the
existing EPL-2.0 crate; the MIT/Apache adapter delegates rather than translating algorithms.
