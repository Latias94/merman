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

## Public normal-path control

On 2026-08-08 (the report's explicit `America/Los_Angeles` timestamp is
`2026-08-07T17:19:42.720813-07:00`), the registered public `complete-svg` Flowchart lane was
run between the direct parent `H` and candidate `C` with serial Cargo, eight A/A calibration
pairs per revision, and ten fresh balanced AB/BA pairs where calibration was stable. This was a
confirmation control, not a latency-admission experiment.

| Fixture | Input bytes / SHA-256 | Base output | Candidate output |
|---|---|---|---|
| `flowchart_long_edge_labels` | 1,141 / `d0b6e45242bacdc28667f507a3f3edecb11543929a762e18cc30f60c2c2dee39` | 33,542 bytes, 222 SVG elements, `96a9c7bd7053d60a44f95dac6d97537d672351c9a6a370e4758244d1da053c78` | identical |
| `flowchart_selfloop_bidi` | 464 / `d4bf4e7928761feded0593e94b6a270e42e2c329b4fad07ee2cab9073f43c1c7` | 38,878 bytes, 260 SVG elements, `e38bea4a0494c62120d3c318c371643d6a712f0944a83c25e688abd41ebed9ea` | identical |

Both revisions used `merman` / `pipeline`, `--no-default-features --features complete-svg`,
Rust/Cargo `1.95.0`, bench profile, target `aarch64-apple-darwin`, and `CARGO_BUILD_JOBS=1`.
The lock, corpus, pipeline harness, manifest, and runner digests were respectively
`419e867cfb829caac42ec394e91be0173775386dd7348abd08076bf9677f4651`,
`49ef1934894455a1d0197edd19ec349b4c32c48941fb904e9e18ac2c793bca2e`,
`a38265f1c72b92a8e5f7952d6cc053434685dcdb237c03b0de84913046fdbc31`, and
`c2685dfd2e0c25c4cf2260160b1aa373633c00e20e5c60f14089ab2ac51f2aa5`.

The JSON audit is 661,163 bytes with SHA-256
`e9aad7c5975c4bbf67f8d9e1d6f9f8200ed5c0a4db6b3b0d4a648c356580225f`; the Markdown companion is
1,827 bytes with SHA-256
`46abb7771822f380a0936f5a2cf4dfaebbe532e7dfdd05876d86d2ffe969e292`. Frozen benchmark
executables were 26,318,320 bytes (`cf0dc6d9bafd4a1ec8f490ce870c90c4ac59055034fd7f40aab88079e343e92e`)
and 26,319,408 bytes (`6090ad5f624cb6f50a48617b6cf56344a46571b108bdffad7571dad0142d7157`).

The self-loop row was stable: ten fresh balanced pairs produced point estimates of 542,255 ns
(base) and 540,205 ns (candidate), with the simultaneous relative bound `-0.71% .. -0.01%`
and absolute bound `-3,860 .. -53 ns`; both outputs were identical. The long-edge row was not
admitted because the base A/A calibration was unstable (`identity_*` and `order_*` equivalence
checks), so it produced no AB/BA pairs. This is measurement noise, not an observed output or
semantic mismatch. We deliberately do not increase sampling after seeing the result; U7 therefore
retains its structural claim and makes no public latency claim.

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
