# FCoSE compound-topology decision — 2026-08-05

## Decision

Status: **accepted-structural**.

FCoSE now builds compound ancestry, edge projections, and owner-graph connectivity once for a
normalized graph and reuses that topology across every run and iteration. The non-grid repulsion
fallback enumerates only pairs that share an owner. Architecture includes this setup in its
render-owned work schedule and can reject an insufficient public budget before topology
allocation.

This receipt makes no end-to-end latency, allocation-count, or peak-memory admission claim. Those
claims require a clean adjacent A/B lane that was not run for this decision.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| base | `8fb06e87358b4974d879de9d04c1648c3c9c4b28` | `ed43a7b0d1bab3a5db4a5f7d84fc669aa6c1741a` | Direct production parent; compound topology was recomputed inside each FCoSE simulation construction. |
| candidate | `d0e889704a86e93131789c75cbcc09f4dc4fa674` | `eae80b4fd596c2c308a3c208a5a27dd16e98eb09` | One reusable `CompoundTopology`, owner-local pair traversal, and render-visible topology work accounting. |
| Mermaid source | `7c0cafcf42e76bfaf79d0cbbd12edb986612f014` | — | Local `repo-ref/mermaid` behavior authority for the pinned Mermaid 11.16.0 source tree. |

The candidate is the direct child of the base.

## Official behavior authority

The implementation follows the companion versions installed by the pinned Mermaid toolchain:

| Package | Version | Source behavior used |
|---|---:|---|
| `cytoscape-fcose` | 2.2.0 | FCoSE orchestration and rerun semantics. |
| `cose-base` | 2.2.0 | Compound gravity and force-layout ownership semantics. |
| `layout-base` | 2.0.1 | Lowest common ancestor, source/target projection in the LCA graph, and owner-graph connectivity. |

The Rust cache stores facts that these sources compute from immutable normalized topology. It does
not change node order, edge order, projected endpoints, gravity eligibility, random-stream reset,
iteration count, or configured algorithm selection.

## Structural bound

Let:

- `V` be the total normalized node count, including compound nodes;
- `E` be the normalized edge count;
- `D` be the maximum containment depth, with `D <= compounds + 1`;
- `R` be the number of FCoSE runs (`1`, or `2` when rerun is enabled); and
- `I` be the configured iteration count per run.

Before the candidate, every simulation construction rebuilt an edge-local ancestor mark vector
and walked parent chains for every edge. It also rebuilt owner connectivity independently for
owner graphs. The reachable topology path included `O(R * E * V)` ancestor/mark work and repeated
`O(V)` temporary storage per edge, plus repeated owner-local connectivity scans. The topology was
also absent from Architecture's complete predictable work schedule.

The candidate performs one topology setup:

- ancestry and immediate-child projection: `O((V + E) log D)` time;
- strict union-find work bound: `O((V + E) log V)` time;
- temporary ancestry space: `O(V log D)`;
- retained edge projections and owner connectivity: `O(V + E)` space; and
- rerun reuse: the topology setup tranche is identical for one run and two runs.

The binary-lifting table uses the observed containment depth rather than `log V`. A flat graph
therefore allocates one ancestry level. The public count-only schedule remains deliberately
conservative when the compound count is unavailable; Architecture uses the graph-aware schedule.

For non-grid repulsion, the fallback now enumerates
`sum_owner(children(owner) choose 2)` pairs in stable owner and child order. It no longer generates
all global pairs and rejects pairs whose nodes have different owners.

## Eight-point work curve

The deep-chain structural control uses one cross-hierarchy edge and containment depths
`8, 16, 32, 64, 128, 256, 512, 1024`. Exact schedule work is:

| Depth | Topology work units |
|---:|---:|
| 8 | 155 |
| 16 | 294 |
| 32 | 593 |
| 64 | 1,244 |
| 128 | 2,663 |
| 256 | 5,746 |
| 512 | 12,413 |
| 1,024 | 26,760 |

Every doubling is increasing and remains below three times the previous point, which rejects the
former quadratic-or-worse ancestry shape. The same test verifies that the deep endpoint projects
to the immediate child below the root LCA, rather than changing compound-edge semantics.

Additional exact structural controls prove:

- one-run and rerun plans charge the same topology setup tranche;
- a balanced union-by-size parent chain is covered by the checked DSU find bound;
- dense owner-local pair work accepts an equal budget and rejects one unit below before traversal;
- a compound graph's complete predictable plan accepts its exact budget and rejects one unit
  below without recording a partial charge;
- flat graphs use one ancestry level rather than a `log V` table; and
- topology arithmetic overflow is reported through the neutral work failure even when a caller
  uses an otherwise unlimited policy.

## Public Architecture boundary

The single-node Architecture fixture with `numIter = 5` and `randomize = false` requires exactly
55 layout work units after topology setup is included:

- `max_layout_work_units = 55` succeeds; and
- `max_layout_work_units = 54` fails in `LayoutModel` with `actual = 55`, `cause = ceiling`, before
  the FCoSE topology is allocated or mutated.

No resource profile ceiling was raised to admit the candidate.

## Verification

Executed serially in the repository's normal `target/` directory:

- `CARGO_BUILD_JOBS=1 cargo nextest run -p manatee --all-features --no-fail-fast` — 64 passed.
- `CARGO_BUILD_JOBS=1 cargo clippy -p manatee --all-targets --all-features -- -D warnings` — passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run -p merman-render --all-features --test architecture_layout_test --no-fail-fast` — 13 passed against the candidate content before unrelated concurrent Flowchart edits made the wider render crate temporarily uncompilable.
- `cargo fmt -p manatee`, focused `rustfmt --edition 2024 --check`, and `git diff --check` — passed.
- Two independent final reviews found no blocking correctness or performance-accounting defect.

## Reproducibility record

| Field | Value |
|---|---|
| host | Apple arm64, Darwin 25.5.0 |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)` |
| nextest | `cargo-nextest 0.9.140` |
| profile | test/dev profile; serial Cargo with `CARGO_BUILD_JOBS=1` |
| candidate FCoSE blob | `710fd613025fff43cda0a529af7ed8570e15f33a` |
| candidate Architecture blob | `7f267764e7686b384cb63c1766595a89ef7355a3` |
| candidate Architecture integration-test blob | `1521c5fd3e00ffa49c38c1717a4513cb2a0d6a48` |
| `cytoscape-fcose` package SHA-256 | `0c2803ff9ff6f99335b41d993d44a64fc419bfec19c042e2a1e3d2cc5ee1fc31` |
| `cose-base` package SHA-256 | `6c83ae210757f78ecdd147d0e4cd6caab1a69ceb2f0c331d69962f1254c8410f` |
| `layout-base` package SHA-256 | `99d1ba119dbd99ca770d7ce03ebc57e6f1721b944ae02d2a1d65cf2843d33ed2` |

The worktree also contained unrelated concurrent Flowchart and text changes. They were excluded
from the candidate commit and were not used as FCoSE evidence. Pre-existing `rust_out` and
`test-results/` paths were not modified, removed, or used as evidence.

## Residual risk and claim boundary

- The old and new implementations were not benchmarked as adjacent prebuilt binaries, so this
  receipt does not attach a latency or allocation percentage to the structural improvement.
- A wider shallow graph, a deep sparse graph, and a deep dense graph remain useful future
  peak-memory controls if an `accepted-memory` claim is desired.
- Third-party callers that use the count-only schedule without a compound count can see a
  conservative false rejection on wide shallow compound graphs. The render-owned Architecture
  path uses the graph-aware API and is not affected.
- Generated/property-based compound forests would broaden the differential proof beyond the
  representative official-semantics fixtures, but no concrete parity defect was found.
