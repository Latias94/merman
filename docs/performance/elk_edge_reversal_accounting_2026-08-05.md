# ELK edge-reversal work-accounting decision — 2026-08-05

## Decision

Status: **accepted-structural**.

The ELK layered pipeline now accounts non-greedy edge-reversal work by the processor that owns
it. Model-order processing charges the exact feedback-edge stream produced by the pinned runtime
order, constraint processing uses an ordered shadow of the pinned two-pass mutation semantics, and
the restorer charges only marked edges against their actual incoming and outgoing lists. This
receipt makes no wall-clock or memory-percentage claim.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `H` | `6464f595b5db0a54dff600f7bce41e2fffdae91f` | `040da5d8b31d1895c43ba037ff774e58b87c6cc0` | Direct production parent with one whole-graph mutation bound for every non-greedy reversal processor. |
| `C1` | `e47625f4de52be4d0953cd009eb6027b0ebdf12b` | `5407b40ac335cc5466dc79fb8878706452dc595f` | Owner-scoped accounting, official-order probes, and initial structural controls. |
| `C2` | `ec999da2a0787893bc90df2a0fb66da3790cb810` | `bb6733d49197f9ac26b64717636a417ce7659cb8` | Final ordered constraint shadow, allocation-floor admission, collector-summary reuse, and regression controls. |

`C1` is the direct child of `H`. `C2` is the final production candidate; the intervening commit is
the first version of this receipt and contains no production changes. Each production change keeps
its owner-local proof tests in the same commit because the public budget result changes with the
estimator.

## Semantic authority

The behavior oracle is the repository-pinned `elkjs 0.9.3` runtime:

- path: `tools/mermaid-cli/node_modules/elkjs/lib/elk-worker.js`;
- SHA-256: `90ab078ad34ff826ca0ece1ee338ae98071a6b3e6cfbb00ce2eb671b32eacd88`;
- relevant runtime owners: `ModelOrderCycleBreaker`, `InteractiveCycleBreaker`,
  `EdgeAndLayerConstraintEdgeReverser`, `ReversedEdgeRestorer`, and `LEdge.reverse`.

The Rust implementation preserves node, port, and outgoing-edge traversal order. In particular,
model-order scoring still consumes the stateful `FIRST_SEPARATE` and `LAST_SEPARATE` counters for
every target evaluation; it is not replaced with a per-node score cache.

## Structural model

Let:

- `V` be the number of layerless nodes;
- `P` be the number of ports;
- `P_max` be the largest port count on one node;
- `A_i` and `A_o` be incoming and outgoing adjacency entries;
- `d_p` be the total incidence of one dedicated port;
- `C_n` be the combined incidence of a node's collector ports;
- `R` be the selected or marked reversal-edge incidence.

Stable `Vec` removal and capacity-independent append/growth remain bounded by the endpoint list
width. For a selected edge, a dedicated endpoint contributes `d_p`; a collector endpoint
contributes `C_n`. The candidate-local adjacency bound is twice the sum of those endpoint widths,
covering stable removal and append/growth. Collector lookup is charged only when reversing from an
output collector or into an input collector. Labels, bend points, endpoint replacement, and the
reversed flag are charged per selected edge.

The processor-specific boundaries are:

- ModelOrder: exact feedback edges from the same stateful visitor used by production.
- Edge/layer constraints: exact direct candidates followed by an ordered shadow of the official
  mutable fixed-side `remainingNodes` pass. The shadow records only processor-local reversed bits
  and current per-port flow, so earlier fixed-side reversals can admit later nodes without cloning
  the full graph.
- ReversedEdgeRestorer: only `reversed = true` edges, with incoming and outgoing list widths
  accounted independently.
- DepthFirst and Interactive: exact no-mutation elision for monotonic-index DAGs; otherwise the
  existing conservative whole-graph mutation bound remains.

Every estimate uses checked arithmetic and depends on logical lengths, never `Vec::capacity()`.
Collector incidence and materialization facts are summarized once per node, removing the former
per-candidate port rescan. ModelOrder and constraint execution first perform a non-consuming linear
floor check, then allocate their bounded `O(V + E + P_max)` planning state, and finally check and
charge the complete tranche once. A floor rejection reports the known lower bound rather than an
uncomputed complete estimate. The public `WorkControl` contract explicitly permits these monotonic
prefix checks, and no input-sized cache is allocated before the first admission check.

## Unrelated-collector control

The mixed fixture contains `n` merged parallel `A -> B` edges that never reverse and one
independent `C -> D` edge that does reverse. For the constraint lane, `A = First` and `B = Last`
exercise directionally irrelevant constraints while `C = Last` owns the real reversal. The old
whole-graph mutation formula for this exact fixture is `4n² + 9n + 95`; both the ModelOrder
candidate stream and the ordered constraint shadow now charge `97` mutation units at every tested
scale.

| Parallel `A -> B` edges | Whole-graph mutation bound | Candidate/owner-scoped bound |
|---:|---:|---:|
| 64 | 17,055 | 97 |
| 128 | 66,783 | 97 |
| 256 | 264,543 | 97 |
| 512 | 1,053,279 | 97 |

At `n = 512`, the former mutation estimate alone exceeded the interactive profile's 800,000-unit
ceiling even though only the independent edge could mutate. The accepted result removes that
false rejection without weakening the reachable shared-collector quadratic bound.

## Additional structural controls

- Dedicated-port stars charge exactly `7E` units and perform no collector lookup or
  materialization.
- A logically identical graph with different spare `Vec` capacities produces identical public
  work units.
- Complete collector IDs, including the full node ID, are charged once per possible opposite-port
  materialization.
- Ordinary unmarked collector DAGs take the linear no-mutation path for constraints and Restorer.
- An all-marked shared collector cycle retains the reachable `8n² + 6n` Restorer bound.
- A shared list with `n` edges and one marked edge charges `4n + 3`, proving that sparse restoration
  uses `marked_count * list_len` rather than `marked_count²`.
- Stateful ModelOrder coverage fixes the official result for two parallel `FIRST_SEPARATE` edges
  and two parallel `LAST_SEPARATE` edges: only the first edge in each ordered pair reverses.
- Direct `First`/`Last` nodes are excluded from the fixed-side second pass, matching the pinned
  runtime's `remainingNodes` boundary.
- A three-node fixed-side cascade proves that reversing `B -> A` can make a later `B` eligible and
  must include the subsequent `C -> B` reversal in the candidate set.
- A direct `First` reversal adjacent to a fixed-side merged collector proves that the fixed node is
  still ineligible and does not pull its 128 unrelated parallel edges into mutation accounting.
- Budgets below the linear planning floor observe one non-consuming check, no charge, and no graph
  mutation for both ModelOrder and constraint planning.
- One unit below the computed budget rejects before mutation and without charging; the exact
  budget succeeds; one unit above produces the same graph with one unit left.

## Verification

Executed serially in the repository's normal `target/` directory:

- `CARGO_BUILD_JOBS=1 cargo nextest run -p merman-elk-layered --all-features --no-fail-fast` —
  315 passed.
- `CARGO_BUILD_JOBS=1 cargo clippy -p merman-elk-layered --all-targets --all-features -- -D warnings`
  — passed.
- Focused red/green controls covered irrelevant direct constraints, fixed-side cascades,
  direct-adjacent ineligible collectors, linear allocation-floor admission, monotonic collector
  DAGs, direction-specific sparse restoration, stateful ModelOrder traversal, and unrelated
  collector regions beside one real reversal owner.
- `CARGO_BUILD_JOBS=1 cargo fmt -p merman-elk-layered -- --check` and focused
  `git diff --check` — passed.
- Independent correctness, performance, testing, and simplification reviews were run against the
  candidate scope. Follow-up adversarial reviews found and verified the fixed-side cascade before
  `C2`; no blocking finding remained at the final tree.

## Reproducibility record

| Field | Value |
|---|---|
| host | Apple M4 Pro, arm64, macOS 26.5.1 build 25F80 |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| nextest | `cargo-nextest 0.9.140` |
| profile | test/dev; serial Cargo with `CARGO_BUILD_JOBS=1` |
| `Cargo.lock` SHA-256 | `4b5dc24ee4037349abe9995997a85ef9ca7ab59164611452526cb7013a8ade6f` |
| `graph.rs` blob | `4e86fc2804f6160dd8c7f613e4cfb1f3795a9806` |
| `intermediate.rs` blob | `252028ac3d4460f37983d80f75d097b41b8f393b` |
| `p1cycles.rs` blob | `a88b22fa5e7acf178779068365f3c80a4752ae20` |
| `pipeline.rs` blob | `c175b9573945cd1b36b256180384b44194d23aa6` |
| `work.rs` blob | `b27a12a879ab96414a26b1d2b8f7987197405fad` |

## Residual risk and claim boundary

- This is an `accepted-structural` result. No adjacent prebuilt-binary latency or peak-memory
  experiment was registered, so no runtime percentage is claimed.
- DepthFirst and Interactive only prove the no-mutation fast path for monotonic-index DAGs. A DAG
  with another valid declaration order can conservatively retain the full mutation bound. An
  allocation-safe exact planner remains a separate candidate.
- Possible collector materialization is still charged as a graph-wide linear term after a
  candidate is found. It cannot reintroduce the removed unrelated quadratic amplification.
- Hand-constructed public `LGraph` values with invalid adjacency indices remain outside the normal
  importer invariant and may fail before meaningful layout work begins.
- Concurrent Flowchart and text changes were excluded from `C1` and `C2` and were not used as
  evidence.
  Pre-existing `rust_out` and `test-results/` paths were not modified, removed, or used as
  evidence.
