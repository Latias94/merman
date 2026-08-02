# Dugong transient-node retirement decision — 2026-08-02

## Decision

Status: **accepted-structural**.

Dugong now retires edge-label proxy nodes and valid self-edge dummy nodes with one
`Graph::remove_nodes` call per phase after all rank or edge-geometry writeback is complete. This
receipt makes no latency claim.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `H` | `c81d76e794aacdd613eb1c57cbd224a6a11b4767` | `dcd31ab447d52f78b22267195802950294e074df` | Direct production parent with sequential transient retirement. |
| `C` | `b192d6f5772ab89a489615ea2e22f370b8e24758` | `25191b5537ff07bdcf1279756e04888a97cf5b0b` | Batch-retirement production candidate. |
| proof | `67964bfbde893b4522983bb725cfe7d3c681d58e` | `a5cfa7c36b8911f120806626944a28bb484dd82a` | Adds eight-point structural curves without changing production behavior. |

`C` is the direct child of `H`. The proof commit is the direct child of `C` and changes only
owner-local tests in the same two Dugong source files.

## Structural bound

Let `T` be the number of live transient nodes retired in a phase, and let `V` and `E` be the
current graph's node and edge counts.

- Before, each live transient called `Graph::remove_node`. Each call obtains an adjacency view,
  mutates topology, and invalidates that view, so a many-node phase has a worst-case
  `O(T * (V + E))` adjacency-reconstruction path.
- After, writeback is `O(T)`, defensive proxy discovery is `O(V)`, and one
  `Graph::remove_nodes` call performs one `O(T + V + E)` graph-wide retirement pass.
- `Graph::remove_nodes` preserves the relative order of surviving nodes, edges, and compound
  children and leaves the graph untouched when target discovery fails before mutation.

The accepted statement is deliberately “one graph-wide removal pass per normal retirement
phase.” It does not claim that every adjacency cache in the complete layout pipeline is rebuilt
exactly once.

## Exactness boundary

Edge-label proxies preserve the former ordering contract:

1. Generated proxies write back first in generated order.
2. Caller-provided defensive leftovers write back afterwards in stable node order.
3. A later leftover targeting the same edge remains last-write-wins.
4. Missing IDs, non-proxy generated IDs, orphan proxies, parented proxies, surviving node order,
   compound-child order, and edge order retain their prior behavior.

Valid self-edge dummies restore all edge labels and points before the batch is committed. Width,
height, extras, coordinates, exact five-point geometry, edge order, node order, and compound-child
order are preserved.

Internally generated self-edge dummies point to ordinary source nodes and always take the batched
path. A malformed caller-constructed graph can make one dummy depend on another. Immediate removal
then changes endpoint recreation and later edge survival, so that defensive case intentionally
keeps the former sequential behavior. The structural claim is limited to valid generated dummies;
the fallback is tested and is not presented as linearized.

## Eight-point structural controls

Both retirement owners run paired legacy-versus-candidate structural controls at
`T = 1, 2, 4, 8, 16, 32, 64, 128`.

| Phase | Legacy live removals | Candidate graph-wide passes | Semantic comparison |
|---|---|---|---|
| Edge-label proxies | `T` at every point | `1` at every point | Exact node order, edge order, and final label writeback. |
| Self-edge dummies | `T` at every point | `1` at every point | Exact node order, edge order, and restored edge labels/points. |

The controls execute the same private helpers used by production. The retirement callback is
`FnOnce`, and the production callback contains one direct `Graph::remove_nodes` invocation, so a
passing test cannot hide a second batch submission behind a mutable counter.

## Verification

- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p dugong`: 296 passed after the proof commit.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p dugong-graphlib`: 108 passed, including batch
  equivalence, order preservation, compound cleanup, and iterator-panic atomicity.
- `CARGO_BUILD_JOBS=1 cargo clippy --locked -p dugong --tests -- -D warnings`: passed.
- Clean `C` worktree Flowchart integration control:
  `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman-render -E 'test(/flowchart/)'` —
  172 passed, 1,059 skipped by the filter.
- `cargo fmt -p dugong` and `git diff --check`: passed.

No adjacent public latency experiment was registered, so there is no `accepted-latency` claim.
The pre-existing untracked `rust_out` and `test-results/` paths were not read, modified, removed,
or used as evidence.
