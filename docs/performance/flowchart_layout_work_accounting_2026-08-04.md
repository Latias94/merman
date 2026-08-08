# Flowchart layout work-accounting decision — 2026-08-04

## Decision

Status: **accepted-structural**.

The Flowchart render path now carries one render-operation work budget through its selected Dagre
or ELK backend. The accepted claim is intentionally limited to the graph-structure and geometry
tranches enumerated below. It is not a claim that every Flowchart CPU instruction, input byte, or
allocation is metered, and it makes no public latency or resident-memory improvement claim.

The final production boundary is commit `6c4b184874389d8f0b840ba2194c21bb1098790a`, tree
`6b1e074ffeeae3b91ce31274dfb025182b81f468`. No public resource profile was raised.

## Revision and source boundary

| Role | Commit or version | Tree or integrity | Meaning |
|---|---|---|---|
| base | `65e294e544fe57ed852c8d23dc18f1a2d3e01a31` | `ff31e8c06708e24aba7423cc5c84cc8ba58193fa` | Direct parent before this Graphlib, Dugong, and Flowchart tranche. |
| Graphlib ordering | `83fef858575c6b2d976b73b18f1544948ec1b780` | `72f57ea40f90c0b096089d82a66058fc034cd738` | Pins JavaScript object-key order and checked Graphlib work bounds. |
| parent batch | `e1685904185e06aba763d6c4565ba02829be78f3` | `53c45cab968741a93a7a5f40cded5449b64c79d5` | Removes redundant root finds from accepted parent assignments. |
| Dugong reconstruction | `2a57648b8125c3ccd2f269df76077e46e02b059d` | `39daa2e7de2c287c0b285e93a0db20db9c6d8f66` | Bounds temporary graph reconstruction and tombstone-aware kernel work. |
| Dagre adapter | `d3f16c665fe5c2a9eed6882937c8edcf7bb159b3` | `862ad1e59d9db98aafe4536b07b97514292f981a` | Introduces the render-to-Dugong work adapter and adapter-owned charges. |
| edge planning | `09f74cc11f904211a78fd491e3f58ed6480fb3ea` | `b57e184044be154e949ac3683719d05ad87ea9a2` | Precharges normalization plans and reuses translation scans for route planning. |
| cluster cleanup | `ae04b06415d02aeb975df7ea7972cf10bd5bccd2` | `b3874c53ef6efbcf4475680e17856eabc021764a` | Removes duplicate Dagre cycle validation and redundant cluster preprocessing. |
| ELK parity intermediate | `c059829ac22239991de7f3753d144df446512d27` | `f0d709836535d0fd6cce69860b3d659153efcdea` | Restores Mermaid-order parent-cycle errors for ELK with a conservative union-find validator. |
| production meter | `ae496d2a79853408fda0f26b531dc513714cc8e3` | `e152a4c374145f584b84c1367bef24535ad1d659` | Passes the render-owned meter into the selected backend and removes the optimistic aggregate preflight. |
| geometry bounds | `28539199e13421b58b1583ebfda738f9a6c864b2` | `c58c4945c484b6fefe5c88ebc66d3c39eb844088` | Streams final geometry bounds and charges every visited geometry item. |
| final candidate | `6c4b184874389d8f0b840ba2194c21bb1098790a` | `6b1e074ffeeae3b91ce31274dfb025182b81f468` | Replaces the ELK union-find intermediate with linear functional-graph cycle validation. |
| Mermaid authority | `11.16.1` at `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf` | local `repo-ref/mermaid` | Parser, FlowDB, adapter order, and observable behavior authority. The relevant Flowchart and shared rendering-util sources are unchanged from 11.16.0. |
| Mermaid Dagre runtime | `dagre-d3-es 7.0.14` | `sha512-P4rFMVq9ESWqmOgK+dlXvOtLwYg0i7u0HBGJER0LZDJT2VHIPAMZ/riPxqJceWMStH5+E61QxFra9kIS3AqdMg==` | Pinned Mermaid dependency that supplies the browser Dagre and Graphlib modules. |
| Dagre port reference | `@dagrejs/dagre 2.0.2` at `ba986662394f8f3ed608717194e5958f3386ce01` | local `repo-ref/dagre` | Standalone layout reference used by the Rust port. |
| Graphlib port reference | `@dagrejs/graphlib 2.2.4` | `sha512-mepCf/e9+SKYy1d02/UkvSy6+6MoyXhVxP8lLDfA7BPE1X1d4dR0sZznmbM8/XVJ1GPM+Svnx7Xj6ZweByWUkw==` | `repo-ref/dagre/package-lock.json` dependency used to check standalone compound-graph behavior. |

Commits between `28539199e` and `6c4b18487` that change Text, Sequence, or the rejected WASM
candidate are outside this receipt's claim boundary. The final clean verification is bound to
`6c4b18487`; the public Dagre timing control is bound separately to the clean, causally scoped
`65e294e54..28539199e` comparison.

## Accepted work ownership

`OperationWorkMeter` remains private to `merman-render`. Lower crates receive only neutral
`WorkControl` callbacks and return neutral interruption or arithmetic-overflow errors. The render
owner alone maps them to the existing typed `max_layout_work_units` contract.

The shared meter itself is not sticky: a rejected charge does not advance usage, and a later
smaller direct charge can still succeed. Sticky behavior is owned by the operation adapters:

- `DagreOperationWorkControl` retains the first adapter or Dugong rejection for the rest of that
  Dagre operation; and
- `ElkOperationWorkControl` independently retains and maps the first ELK-side rejection.

All admitted products and sums use checked arithmetic. Overflow is a resource error even for the
trusted-input profile, rather than a wrap, clamp, or algorithm substitution.

## Dagre and Graphlib accounting

Graphlib node and compound-child enumeration follows pinned JavaScript `Object.keys` behavior:

1. array-index keys enumerate in ascending numeric order;
2. ordinary string keys retain property-creation order; and
3. deletion and reinsertion preserve the corresponding observable order changes.

Array-index mutation pays a checked ordered-map term. Ordinary keys do not pay a logarithmic term
that the implementation cannot incur. Snapshot work uses slot counts where iterators visit deleted
ordinary entries or edge tombstones.

Flowchart collects final child-parent assignments in pinned Mermaid order and replays them through
Graphlib's batched unparented-parent operation. Let:

- `S` be the node-slot count;
- `E` be the number of existing parent links;
- `P` be the number of new assignments;
- `N` be the number of array-index children among those assignments; and
- `B = ceil(log2(S)) + 1` be the conservative union-by-rank find bound.

For a non-empty Dagre/Graphlib batch, the admitted upper bound is:

```text
6S + 2P + 4B(E + P) + 2BN
```

An empty batch charges zero. Accepted assignments join already-resolved roots directly, removing
two redundant zero-depth finds per assignment. The registered 250-node compound fixture records
`10,620` owner-local Dugong parent units, below the unchanged local `250,000` control ceiling. That
number is not an end-to-end admission claim for the public render meter.

Dugong reconstruction additionally charges temporary graph copies, numeric-key updates, edge and
node slot scans, stable normalization plans, and final route-point work at the owner that performs
them. The translation bounds scan now also creates final route keys, avoiding another complete
edge-slot traversal.

## ELK parent-cycle validation

ELK does not materialize Mermaid's Graphlib compound graph, so Merman preserves the same invalid
model contract in the ELK adapter. Let:

- `H` be the total number of subgraph membership entries;
- `M` be the number of subgraph records;
- `P` be the number of deduplicated subgraph-child assignments that have a final parent; and
- `C = 2P` be the temporary node-capacity upper bound.

The charged tranches are:

```text
parent-map construction: H
assignment scan:         M
non-empty validation:    12C + 4P = 28P
```

The resource expression is strictly `O(H + M + P)`. Runtime is expected `O(H + M + P)` under the
repository's expected-constant-time hash-table model, with `O(M + P)` temporary space. This is a
container-operation work bound, not a CPU-instruction, string-byte, or adversarial-hash bound.

The validator treats the final parent relation as a functional graph. Kahn peeling removes every
acyclic node. For each remaining directed cycle, the edge with the largest Mermaid assignment
index is exactly the edge that would first close that cycle in sequential Graphlib `setParent`.
Taking the smallest such closing index across all cycles therefore returns the global first
Graphlib error. Repeated subgraph IDs replay the same final FlowDB relation, so retaining the first
occurrence cannot change the first observable error.

The earlier `c059829ac` union-find implementation preserved that error but charged a conservative
`P log P` term. At `28539199e`, the deep edgeless-hierarchy work increments were `796` and `924`
for equal depth steps; the exact `128` difference came from that logarithmic ceiling. The final
functional-graph implementation restores equal linear increments without weakening the charge.

## Cluster and geometry changes

The accepted Flowchart cleanup:

- borrows stable subgraph indexes instead of rebuilding equivalent maps;
- reuses recursive-child collections instead of rediscovering them;
- skips zero title-shift traversals;
- removes Dagre's duplicate cycle preflight now that Graphlib owns that decision; and
- computes final node, cluster, edge-point, edge-label, and terminal-label bounds in one streaming
  pass.

The final bounds pass uses constant-size accumulators, so it removes the geometry-point staging
vector and adds `O(1)` accumulator space. This is not represented as an RSS improvement.

## Mermaid behavior authority

The official source-backed behavior is:

- FlowDB iterates subgraphs in reverse order to build the final parent map and emits group nodes in
  that reverse order.
- The Dagre renderer consumes emitted nodes in order and calls Graphlib `setParent` immediately.
- Repeated subgraph IDs therefore replay one final parent relation rather than forming a general
  reparent sequence.
- Cluster nodes precede leaf vertices, while numeric object keys still follow JavaScript
  enumeration rules.
- Cluster edge rewriting uses a pre-mutation edge snapshot and remove/reinsert order.

Mermaid's external ELK renderer does not itself throw Graphlib's parent-cycle error. Preserving the
same first invalid-model error across Merman's Dagre and ELK backends is an explicit Merman parity
contract derived from FlowDB and Dagre behavior, not a claim about ELK's own browser wrapper.

The former legacy-oriented assertion is not retained. The protected tests are
`parent_batch_matches_mermaid_graphlib_assignment_order` for Dagre/Graphlib and
`flowchart_elk_parent_validation_reports_mermaids_first_assignment_cycle` for ELK.

## Evidence time authority

The authoritative current date for this receipt is **2026-08-07**. The local machine clock moved
ahead while the final public controls were generated, so the ignored JSON reports and the harness
commit contain automatic `2026-08-08` timestamps. Those future timestamps are excluded from the
chronology. Git ancestry, source trees, executable SHA-256 values, fixture digests, command lines,
and report digests are the evidence authorities.

## Structural and semantic verification

All Cargo commands below ran serially with `CARGO_BUILD_JOBS=1` in a clean detached worktree at
`6c4b18487`, tree `6b1e074f`, using Rust/Cargo `1.95.0` and cargo-nextest `0.9.140`:

- default Flowchart Nextest: `187` passed, `1,077` skipped;
- Flowchart Nextest with `layout-elk`: `222` passed, `1,090` skipped;
- complete `dugong-graphlib` Nextest: `135` passed;
- complete `dugong` Nextest: `327` passed;
- `cargo check --locked -p merman-render --tests --features layout-elk`: passed;
- owner-local `merman-render` Clippy with `layout-elk`: passed with the two documented pre-existing
  crate-local lint allowances;
- owner-local `dugong-graphlib` Clippy: passed;
- owner-local `dugong` Clippy: passed with the pre-existing `too-many-arguments` allowance;
- `cargo fmt --all -- --check`, `git diff --check`, and clean-worktree verification: passed.

The exercised contracts include exact below/equal/above budget boundaries, non-advancing failed
charges, sticky adapter rejection, checked overflow, valid-output equality, final streamed bounds,
the 8,192-assignment parent batch, and the separate 10,000-depth Dagre cluster run on a 512 KiB
worker stack.

An independent, non-Cargo review harness compared the final ELK functional-graph result with a
sequential ancestor-walk reference over 948,524 unique-child cases and 304,720 repeated-same-
relationship cases for graphs up to five IDs. It found zero mismatches. That harness was not added
to the repository and is supporting review evidence rather than a continuous regression gate.

## Representative public non-regression control

The registered comparison uses the clean causal range `65e294e54..28539199e`, before later ELK,
text, Sequence, and resource-profile changes. Both sides used independently rebuilt and frozen
full-SVG executables:

- base executable SHA-256:
  `1aebba178c072bfa136a5978bbd9228ee9e11933e4da263a6b42c4d5e8c3528c`;
- candidate executable SHA-256:
  `f45b3bc7ea776f0d15a0d49c17080591d8f23223d767226c4d627ed3898809a0`.

Input fixtures were byte-identical across both revisions:

| Fixture | Bytes | SHA-256 |
|---|---:|---|
| `flowchart_medium` | 1,474 | `b65d43d4c67df71bc188829344583b977b3c2a805e5649d711d849a7a56d6572` |
| `flowchart_nested_clusters` | 725 | `29e4a3e13446e123a77064cbc07fbe3ae6b8dbfdcc7fe842649b050909d4e2f1` |

Each lane used eight balanced A/A calibration pairs per side, eight fresh alternating AB/BA
confirmation pairs, 30 Criterion samples, a two-second warmup, a three-second measurement window,
10,000 bootstrap resamples with seed `20260807`, simultaneous 95% Bonferroni bounds, and the joint
10% plus 50-microsecond regression threshold.

| Public control | Base | Candidate | Paired change | 95% relative interval | Result |
|---|---:|---:|---:|---:|---|
| `end_to_end/flowchart_medium` | 2,043,700 ns | 1,989,187.5 ns | -2.604% | -4.733% to -1.151% | confirmed non-regression |
| `end_to_end/flowchart_nested_clusters` | 883,158.8 ns | 910,595 ns | +3.086% | +1.534% to +4.718% | confirmed non-regression |

The medium absolute interval was -100,962.5 to -23,012.5 ns. The nested-cluster absolute interval
was +14,091.5 to +42,388.0 ns, remaining below the 50-microsecond gate. Output identity matched
exactly:

| Fixture | SVG SHA-256 | Bytes | Elements |
|---|---|---:|---:|
| `flowchart_medium` | `4a2b2d89ef3ffbb67a1524051b95a8658fde9e336798aed3154dc3ec6dfa9f38` | 75,702 | 683 |
| `flowchart_nested_clusters` | `e03c9aa6b02baae36175c2624a5e7ba6f69e2003115389ec8342218f4dc73921` | 33,856 | 299 |

Raw evidence:

- medium discovery SHA-256:
  `c500d87e6d06a38934ffa3a391915b6b88880b37f399415a35e212dc53373e4c`;
- medium confirmation SHA-256:
  `8ad17e1729a06d8dbfaeadafe0640e2edc4d3a0aacba92776fbdd702bb3d3301`;
- nested discovery SHA-256:
  `8835a2401c74764e59a1c8dc96a6cff014b4987c1e2aad01e077f350e2cc907b`;
- nested confirmation retry SHA-256:
  `fccbd1d725baa94ff052c4fe2fd5c058e8144b14af708731a1f7f50b3762632d`.

The first nested confirmation was inconclusive because its A/A absolute and order bounds did not
fit the eight-pair cap. The one permitted identical-protocol retry passed. The inconclusive report
is retained with SHA-256
`9456622bb65c60263ef45988b226c928c4a25530759a833cb164dcf661efd691` and contributes no result.

## Evidence-tool repair and rejected attempts

Commit `724d230a4214c041308c81897f365bf4ca71e970` scopes a single-lane Criterion
discovery to the exact registered benchmark. Its source SHA-256 is
`0d83f0f1e00ccea474c7e6767589c64df7153f68a64a0df97f3414dd76cb7fdf`.
This candidate-neutral repair was required because the historical pipeline harness otherwise
preflighted every registered benchmark during `--list` before applying the comparison selection.

Two discovery attempts were rejected before sampling:

1. The first enabled only `svg`; all-benchmark discovery reached an unrelated Mindmap fixture and
   failed because `layout-cytoscape` was unavailable. The invalid report SHA-256 is
   `4e942e3ab9aeb55acab7ded4c8a243150e0cab9facbf6443bfe3e5e611be0d3e`.
2. The second enabled the complete SVG capability set, but all-benchmark discovery reached
   `flowchart_large` on the historical candidate. Its newly accounted 258,288 work units exceeded
   the then-current 250,000 default before either selected lane was measured. The invalid report
   SHA-256 is
   `ed9df80679abad148356a27eebaaa44ae5a5ddc559b7b3aa4f10a71510b0404c`.

Neither failed report contains performance evidence. Exact discovery preserves the original
resource profile and validates only the two preregistered public controls rather than bypassing a
selected-input rejection.

## Residual risks and non-claims

The following work is deliberately not hidden by this receipt:

- Flowchart label parsing, Markdown/HTML conversion, text measurement, string hashing/copying, and
  browser font behavior are outside complete layout-meter coverage.
- Defensive self-loop merge code traverses segment points and hints again without a dedicated
  point-count tranche for that second traversal.
- Cluster copy performs an edge-slot snapshot per copied node and linear descendant membership.
  For `N` copied nodes, `Es` edge slots, and `D` descendants, the reachable bound can be
  `Theta(N * Es * D)`, including `Theta(V^2 E)` shapes.
- Cluster adjustment can reach `Theta(E * sum(descendants_per_cluster))`; nested membership can
  make that descendant sum `Theta(V^2)`.
- Common-edge discovery performs incident-list cross-comparisons that can reach `O(E^2)` per call
  and may run repeatedly.
- Retaining descendant vectors for all nested clusters can require `Theta(V^2)` memory.
- Numeric JavaScript object keys intentionally retain their ordered-map logarithmic term.
- ELK scoped-edge and materialized-section work remains output-sensitive, and the ELK kernel keeps
  its algorithm-specific costs.
- Browser text measurement, `getBBox()`, `foreignObject`, font rasterization, and DOM wrapper noise
  remain separate parity residuals.

These are the next evidence-backed optimization candidates; none is described as the unique or
largest remaining hotspot without a profile proving that ranking.

The pre-existing untracked `rust_out` and `test-results/` paths were not read, modified, removed,
or used as evidence.
