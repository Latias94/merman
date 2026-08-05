# ELK edge-reversal work-accounting decision — 2026-08-05

## Decision

Status: **accepted-structural**.

The ELK layered pipeline now accounts non-greedy edge-reversal work by the processor that owns
it. Model-order processing charges the exact feedback-edge stream produced by the pinned runtime
order, constraint processing charges only edges attached to possible reversal owners, and the
restorer charges only marked edges against their actual incoming and outgoing lists. This receipt
makes no wall-clock or memory-percentage claim.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `H` | `6464f595b5db0a54dff600f7bce41e2fffdae91f` | `040da5d8b31d1895c43ba037ff774e58b87c6cc0` | Direct production parent with one whole-graph mutation bound for every non-greedy reversal processor. |
| `C` | `e47625f4de52be4d0953cd009eb6027b0ebdf12b` | `5407b40ac335cc5466dc79fb8878706452dc595f` | Owner-scoped accounting, official-order probes, and structural controls. |

`C` is the direct child of `H`. Production code and its owner-local proof tests are intentionally
kept in one commit because the public budget result changes with the estimator.

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
- Edge/layer constraints: edges whose source or target can own a direct layer-constraint reversal
  or the official fixed-side `remainingNodes` pass. Unrelated collector regions are excluded.
- ReversedEdgeRestorer: only `reversed = true` edges, with incoming and outgoing list widths
  accounted independently.
- DepthFirst and Interactive: exact no-mutation elision for monotonic-index DAGs; otherwise the
  existing conservative whole-graph mutation bound remains.

Every estimate uses checked arithmetic and depends on logical lengths, never `Vec::capacity()`.
No input-sized cache is allocated before the processor budget check.

## Unrelated-collector control

The mixed fixture contains `n` merged parallel `A -> B` edges that never reverse and one
independent `C -> D` edge that does reverse. The old whole-graph mutation formula for this exact
fixture is `4n² + 9n + 95`; both the ModelOrder candidate stream and the constraint owner scope now
charge `97` mutation units at every tested scale.

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
- One unit below the computed budget rejects before mutation and without charging; the exact
  budget succeeds; one unit above produces the same graph with one unit left.

## Verification

Executed serially in the repository's normal `target/` directory:

- `CARGO_BUILD_JOBS=1 cargo nextest run -p merman-elk-layered --all-features --no-fail-fast` —
  312 passed.
- `CARGO_BUILD_JOBS=1 cargo clippy -p merman-elk-layered --all-targets --all-features -- -D warnings`
  — passed.
- Focused red/green controls covered irrelevant direct constraints, monotonic collector DAGs,
  direction-specific sparse restoration, stateful ModelOrder traversal, and unrelated collector
  regions beside one real reversal owner.
- Focused `rustfmt --edition 2024` and `git diff --check` — passed.
- Independent correctness, performance, testing, and simplification reviews were run against the
  four-file candidate scope. Their blocking findings were resolved before `C`.

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
| `intermediate.rs` blob | `b72e6f089be63a16830fe9ad9bb8e8d0af5eb0a4` |
| `p1cycles.rs` blob | `a88b22fa5e7acf178779068365f3c80a4752ae20` |
| `pipeline.rs` blob | `1f42ef6d2416c0a8bd38d36e0c2fbf97f0523a42` |

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
- Concurrent Flowchart and text changes were excluded from `C` and were not used as evidence.
  Pre-existing `rust_out` and `test-results/` paths were not modified, removed, or used as
  evidence.
