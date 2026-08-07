# ELK hierarchy-preparation decision — 2026-08-03

## Decision

Status: **accepted-structural**.

The headless ELK path now prepares `SeparateChildren` hierarchy scopes from one stable ownership
index and executes them in explicit postorder. It no longer recursively clones every remaining
graph suffix or repeatedly rediscovers descendants. This receipt makes no end-to-end latency or
resident-memory admission claim.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| base | `a25c8caa42ebf277f5492c2d3f7f2e7e112ec922` | `20f82c78372630b086271676a8070cf2b3d1b6a1` | Direct production parent with recursive suffix-graph preparation. |
| candidate | `949ad97481ae9f84decbb4d27695f1bb614b0d6f` | `243fad7c1a6cd6068d72ee6f35f7601b958f4acb` | Stable hierarchy ownership, indexed import, explicit compound traversal, and bounded ELK work. |
| Mermaid source | `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf` | — | Local `repo-ref/mermaid` behavior authority, pinned Mermaid 11.16.1 tree. |

The candidate is the direct child of the base.

The relevant upstream
`packages/mermaid-layout-elk/src/render.ts` source is byte-identical between Mermaid 11.16.0 and
11.16.1, with SHA-256
`70242b274a16fa5e4580396d9778453bb8a18987a43c86953cd4dbdc67794416`. The current 11.16.1
checkout is therefore the active behavior authority without changing this decision's ELK adapter
semantics.

## Normal-path control boundary

The original candidate pair predates registration of a representative public ELK benchmark. A
candidate-neutral harness commit was therefore added to the original base, then the complete
original candidate diff was replayed unchanged:

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| control base | `b1688e23627de43d16c076bf1b7133c58d98bcf1` | `5b7e9b0ded5fe34417300f733822fa2ffc9aa9a3` | Original base plus one existing pinned-Mermaid fixture registration. |
| control head | `5ad67a108b104460816b57686ca639cce289ca32` | `3f498b0a13fcc4419e168c8c33e97d4a9f540a5e` | Direct child containing the complete original U6 candidate diff. |

The full-index original and control candidate diffs have the same SHA-256,
`07a5c69dfa8db1cf0c19e8f3b99354213b67db09ba3d9b233b536249e74df039`, and the same stable
patch ID, `5bd15cf8e155b2b576f7ccb68c5af8f7cb4e20f2`. The harness-only diff has SHA-256
`f35131de950dd07a0679db1065c0663440bf08c323a92bbbba71c8fdd60ac54d`; neither candidate side
changes the two harness files.

The registered public lane is
`end_to_end/flowchart_elk_nested_directions`, backed by
`fixtures/flowchart/upstream_cypress_flowchart_elk_spec_2050_elk_handling_of_different_rendering_direction_in_subgraphs_036.mmd`.
The 303-byte fixture has SHA-256
`2cb57e51813ecf06fbc53962849a62930dbe50b31977bec5aef7de4897db99cb` and exercises nested
`flowchart-elk` subgraphs with explicit direction changes and `SEPARATE_CHILDREN` behavior.
Commit `a110f37a3465dd86f3cda17622a6f504c39f741d` retains the same neutral lane in the active
performance branch so the decision remains reproducible without checking out the synthetic pair.

## Variables and claim boundary

Let:

- `V` be the number of source nodes;
- `E` be the number of source edges;
- `G` be the number of materialized hierarchy scopes, including the root;
- `H` be the number of direct parent-child containment links, with `H <= V - 1` for a forest;
- `D` be the maximum hierarchy depth;
- `C` be the number of hierarchy-boundary edge sections that must be materialized;
- `S` be the number of already-scoped edge-segment descriptors presented to the importer;
- `P` be the number of path components retained in hierarchy-edge output or consumed while
  materializing scoped pieces; and
- `K` be the number of exported geometry records, including points and labels.

`G <= V + 1`, and `C` and `P` are output-sensitive: a single edge crossing a depth-`D` hierarchy
can legitimately require `Theta(D)` boundary/path records. The accepted claim removes avoidable
rescans and suffix copies; it does not pretend those required outputs are constant-size. Across
all edges, both terms can reach `O(E * D)`.

## Removed amplification

At the base, `layout_layered_recursive` first cloned its complete input graph. For every separating
child, `prelayout_separate_children_under` called `graph_for_recursive_child`, whose
`descendant_node_ids` scanned the remaining node array once to seed the search and again for every
discovered descendant. A suffix containing `n` nodes therefore had a reachable `Theta(n^2)`
descendant-discovery path even when `E = 0`.

For a depth-`V` chain in which every non-leaf group separates its children, the wrapper recurrence
was:

```text
T(n) = T(n - 1) + Theta(n^2)
```

so hierarchy preparation had a reachable `Theta(V^3)` time bound. Parent frames also retained
their cloned suffix graphs while a child was laid out, making the sum of live suffix-node and
owned-string copies `Theta(V^2)` on the same chain, before counting required layout output.

At the candidate, `HierarchyIndex` assigns every source node to exactly one direct scope, assigns
every edge to exactly one owning scope, records child scopes in source order, and builds one
explicit postorder. A parent materializes only its direct members, its owned edges, required
boundary sections, and collapsed child extents. Assuming expected constant-time hash-table
operations, the scope-planning and scoped-piece bound is therefore:

- expected `O(V + E + H + G + C)` time; and
- `O(V + E + H + G + C)` additional space.

Because `H <= V - 1` and `G <= V + 1`, both reduce to expected `O(V + E + C)` time and
`O(V + E + C)` additional space respectively.

The importer is a distinct stage: its indexed planning is expected
`O(V + E + S log V)`, followed by output-sensitive `O(P + C)` materialization. After each scope
has already completed source export, the wrapper's `flatten_scope_layouts` pass is
`O(G + E + C + K)`. The earlier source export retains a stable model-order comparison sort with an
`O(E log E)` upper bound. These bounds exclude the algorithm-specific ELK kernel itself.

No recursive suffix graph, descendant collection, or completed child layout is copied into every
ancestor scope.

## Additional ELK-owner structural repairs

The same accepted change set removes several adjacent input-amplified paths without broadening the
semantic boundary:

- Parent-cycle validation previously restarted from every node and used linear `Vec::contains`
  checks along each ancestor path. A depth-`V` chain therefore admitted a `Theta(V^3)` validation
  path. The three-state indexed traversal now finalizes every acyclic path once in expected
  `O(V)` time and `O(V)` space; only the parent-ID lookup relies on hashing.
- Import graph lookup previously recursively searched the nested `LGraph` tree for a parent on
  each access and repeatedly inserted at the front of a path vector. Stable graph slots and
  owner-local node positions make slot addressing worst-case `O(1)` after indexing; ID and port
  map lookups are expected `O(1)`.
- Merged hierarchy ports and inside-self-loop ports previously rediscovered nodes and ports by
  scans. Operation-local owner-bound maps now provide expected `O(1)` reuse lookup. Dedicated
  non-merged ports bypass that reuse index entirely.
- Compound execution now detaches nested graphs into one explicit arena, plans source-ordered
  postorder once, and restores the complete hierarchy through an unwind-safe RAII guard. The
  arena/traversal bookkeeping is `O(G + V)`; edge, port, boundary-section, and individual ELK
  processor costs retain their own separately charged bounds.
- Long-edge split/join preparation now builds owner-local incidence/index state once rather than
  repeatedly scanning global incoming-edge collections. Structural curves cover disjoint chains
  and fan-in cases; this receipt does not translate those counters into a latency claim.

## Work-control contract

Importer planning and output materialization are separate pre-execution tranches. Before building
the containment/hash/heavy-light indexes, the importer charges the conservative planning bound:

```text
V + 3E + S + 4S(L + 1), where L = ceil(log2(max(V, 1)))
```

The terms are one unit per input descriptor, two constant-time owner queries per original edge,
and a safe heavy-light ceiling for one LCA plus two lifts per scoped segment. The importer then
charges the exact future `P`, materialized-section, and label-selection work before allocating
those outputs. Test-only probes demonstrate that observed hierarchy queries stay below the
precharged ceiling.

This is intentionally a resource bound rather than an exact CPU-instruction counter. Near a work
limit, the candidate may reject an input earlier than the base; that is the deliberate cost of
charging derived work before it becomes observable instead of retroactively billing work already
performed.

## Mermaid and ELK behavior authority

The local Mermaid authority is
`repo-ref/mermaid/packages/mermaid-layout-elk/src/render.ts` at the revision above. Its relevant
behavior is:

1. `addVertices` recursively builds one nested `elkGraph` in adapter node order.
2. The root starts with `elk.hierarchyHandling = INCLUDE_CHILDREN`.
3. For a non-empty subgraph, `buildSubgraphLayoutOptions` applies subgraph spacing and
   inside-top-center label placement; an explicit subgraph direction changes that subgraph to
   `SEPARATE_CHILDREN`. Empty groups retain their render label without entering this layout path.
4. `addEdges` appends adapter edges to the root `elkGraph.edges` array.
5. For a cross-subgraph edge, `findCommonAncestor` and `setIncludeChildrenPolicy` enable the
   endpoint ancestry required for ELK to route through the common containing graph.

Mermaid keeps adapter edges at the root; it does not itself store them in a common-ancestor graph.
The Rust/ELK importer materializes stable nested scope ownership, source node order,
common-containing-scope ownership, and explicit scoped pieces needed by the headless layered
pipeline. It does not emulate browser-only D3/DOM behavior such as `getBBox()`, `foreignObject`,
font rasterization, or wrapper noise.

One non-obvious ordering invariant is documented at the production emission site. ELK establishes
an edge's model order from its containing graph before an ancestor-to-descendant edge is moved into
an endpoint's inner graph. A direct parent-child edge can therefore be owned by the parent scope
without materializing a center segment there. Final output must follow `ScopePlan::owned_edges`,
not the scope in which a segment is first observed. Parent-to-child and child-to-parent regression
tests cover this case.

The compound detachment guard carries a second source-alignment comment: Eclipse ELK keeps a
nested graph attached through recursive processing, while Rust temporarily moves it only to
satisfy ownership. Normal errors and panics must therefore restore the complete chain before the
operation leaves the boundary.

## Structural evidence

The production owners expose exact counters only through tests or the existing resource meter.
The accepted controls include:

- `separate_children_hierarchy_index_has_unique_linear_ownership` over depths
  `8, 16, 32, 64, 128, 256`, proving one scope owner per source node and one postorder entry per
  scope;
- `separate_children_wrapper_work_is_linear_in_unique_items`, proving exact
  `HierarchyIndex::build + materialize_scope` work of three node-local units per node for the
  edge-free fixture, not complete layout or export cost;
- `hierarchy_index_work_is_linear_in_emitted_boundary_segments`, proving
  `C = E * (depth + 1)` for the registered fixture and charging each emitted section once;
- `flatten_work_is_linear_in_points_at_fixed_cross_scope_cardinality`, isolating geometry growth
  after edge cardinality is fixed;
- a depth-256 deterministic layout and graph-dump run on a small-stack worker;
- compound-arena curves proving exactly `G` entries and algorithm slots, plus `G - 1` parent links
  and `G - 1` child links;
- `scoped_hierarchy_query_observations_fit_the_precharged_hld_bound`, covering light branches and
  separate forests;
- merged-port controls with exactly `2E` lookups, two creations, and `2E - 2` hits for repeated
  cross-hierarchy edges, plus zero reuse-index activity for dedicated ports;
- `parent_to_direct_child_edge_keeps_owner_scope_order_when_flattened` and
  `direct_child_to_parent_edge_keeps_owner_scope_order_when_flattened`;
- an injected work-control panic proving the detached hierarchy is fully restored; and
- LongEdge split/join structural curves over disjoint and fan-in fixtures.

## Verification

- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-elk-layered`: 285 passed before the
  final heavy-light-bound proof was added; that new proof then passed independently.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-layout-elk`: 38 passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render --features layout-elk`: 1,298
  passed, 3 skipped.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-elk-layered --all-targets -- -D warnings`:
  passed after the final proof.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p merman-layout-elk --all-targets -- -D warnings`:
  passed.
- An archived staged snapshot passed `cargo check -p merman-elk-layered --tests` and
  `cargo check -p merman-render --features layout-elk --tests`.
- `cargo fmt --all`, `git diff --check`, and the staged diff check passed.

The 2026-08-07 normal-path control used Rust 1.95.0 on an Apple M4 Pro with one Cargo build job,
shared-target bench-profile freezing, the `svg,layout-elk` feature closure, eight balanced A/A
pairs on each revision, and eight fresh balanced AB/BA pairs. The evidence used a 5% relative cap,
a 50 microsecond absolute cap, 95% simultaneous confidence with two Bonferroni components, 10,000
bootstrap resamples, and seed `20260807`.

- independently recomputed base A/A noise: `1.180393%`;
- independently recomputed head A/A noise: `0.637069%`;
- derived structural relative budget: `5%`, with both noises below the 3% inconclusive boundary;
- base median estimate: `351,531.25 ns`;
- candidate median estimate: `346,805 ns`;
- paired relative estimate: `-1.334814%`, with a 95% simultaneous regression upper bound of
  `+0.531097%`;
- paired absolute estimate: `-4,726.25 ns`, with a 95% simultaneous regression upper bound of
  `+1,746.25 ns`; and
- matched public output: SVG SHA-256
  `923c8207682e86c1f7aec4c72252bf59213039b33ad518afa10af87dfb09cb1d`, `18,645` bytes, and
  `139` elements on both sides before and after sampling.

The runner classified the row as `confirmed_non_regression` and
`confirmed_non_improvement`. The frozen executables are distinct and immutable: base SHA-256
`ebb72cf69b2e8538ecbd9efb848030b85cc5e7c65070ed9991ae523622f8ef19` (`21,782,016` bytes) and
head SHA-256 `47354d4db3a1710a1256529685ceeb361abc28dedfc8b6e0ab918dfdb19a7d62`
(`21,995,456` bytes). The decision-grade JSON was generated at
`2026-08-07T23:34:37.885187+00:00`; it is `242,774` bytes with SHA-256
`a06f5bdb9e66ea87f3fc196cdbac0c096cbf0e3fb6b5c090c07f53c705c2a707`. Its Markdown projection
is `1,721` bytes with SHA-256
`c78bbb5254fe373e65d8e970c891fc5004833e61d2c1939344335915e246c9ab`.

The passing normal-path control closes the structural regression requirement only. Because the
candidate was not preregistered as a latency optimization and the improvement interval crosses
zero, there is no `accepted-latency` claim. The native-memory lane added around this change is a
non-admission smoke/control surface; RSS alone is not used to claim the `Theta(V^2)` transient-copy
removal as a measured memory win.

## Residual risks and deliberate deferrals

- The conservative hierarchy-query ceiling can reject a near-budget input earlier than an exact
  per-probe counter. Per-probe charging was rejected because the production meter uses atomic
  state and would add a compare-and-swap to every otherwise small indexed query.
- Browser text measurement and SVG DOM details remain bounded parity residuals and are not hidden
  by comparator normalization.
- A single arena spanning every compound preprocessing mutation was considered and deferred. The
  official importer roots pending hierarchy edges and materializes scope-local pieces immediately;
  combining all preprocessing passes would increase ownership and merge-metadata risk without a
  demonstrated superlinear production path.
- The complete ELK kernel still contains algorithm-specific costs. The accepted bound covers
  hierarchy preparation, orchestration, and the specifically indexed importer/LongEdge paths, not
  every layout phase.

The pre-existing untracked fixture probes, `in95p`, `ireframep`, `rust_out`, and `test-results/`
paths were not modified, removed, or used as evidence.
