# LSP semantic-token scheduling repair (2026-09-17)

## Decision and scope

Accepted as a scheduling and resource-accounting repair against `3829707af`, on branch
`perf/label-scan-and-math-measurement`. No end-to-end latency, throughput, p99, or memory reduction
is claimed. The experiment ledger and raw logs are local under
`target/bench/experiments/lsp-syntax-scheduling-2026-09-17/`.

Semantic-token full, delta, and range requests previously performed synchronous source indexing,
Tree-sitter capture queries, and token projection inside the async handler's poll. The bundled
transport polls handlers through `buffer_unordered` alongside input, output, and lifecycle futures
in `join4`. A long semantic-token computation therefore prevented sibling control futures from
making progress, including the input path needed to receive cancellation.

The complete computation closure now runs through `spawn_blocking`, using the existing analysis
executor's shared budgets. With C = 2 CPU permits and P = 8 task slots, syntax and structural
analysis together retain at most C CPU permits and P admitted slots. Syntax requests acquire both
permits before spawning; requests waiting for CPU hold a task slot but do not spawn another worker.
Both permits remain owned by the physical blocking work until it returns or unwinds, even if the
request future has already been dropped. This bounds the newly offloaded work without creating a
separate pool, registry, or cache. It does not bound unrelated runtime users or change the existing
transport admission limits.

Each request uses a child cancellation token. Dropping a request cancels that child without
invalidating the document generation or sibling requests. Session termination drops waiting query
futures and cancels active children. The existing commit barrier still verifies session activity,
document identity, epoch, and syntax cancellation before storing or returning results. Client
capabilities, token projection, full/delta fallback, range filtering, and result identities retain
their existing behavior.

## Evidence

A deterministic regression test joins a semantic-token query and a control future in the same
current-thread task. The computation announces entry and waits for the control future's release
signal. The inline implementation failed this test; the offloaded implementation passes. The
watchdog only bounds failure and is not used to assert a speedup.

Additional deterministic tests hold real synchronous workers behind release channels and check:

- request cancellation leaves the document generation usable;
- two aborted workers retain both CPU permits until their closures exit, and queued syntax and
  structural work share the same capacity;
- worker panic becomes an internal error and releases capacity;
- session termination rejects queued computation and the test shutdown observer waits for real
  worker exit;
- document edits invalidate queued syntax work before its computation can start.

Existing semantic-token and service tests cover full/delta/range behavior, client capabilities,
stale successful and error results, and the termination commit barrier. Independent source review
found no blocking correctness issue.

## Limits

Cancellation is cooperative: a synchronous stage without an internal checkpoint may continue
until it returns. Its permits remain held throughout. Document edits do not directly wake a
request waiting for task/CPU capacity; the request rejects its stale snapshot after obtaining
capacity. Explicit request cancellation and session termination can drop those waits immediately.

Thread handoff and capability-profile cloning add fixed costs to small requests. No ordinary
request latency improvement or non-regression timing claim is made. Token indexing, projection
algorithms, rendering, runtime-neutral core APIs, and dependency declarations are unchanged.

## Validation

On Windows 11 with the repository-pinned Rust toolchain:

- The same-task regression test failed on the inline implementation and passed after offloading.
- Full `cargo nextest run --locked -j 2 -p merman-lsp --features stdio --cargo-quiet`:
  317 passed, including stdio binary and service protocol tests.
- After adding the queued-document-edit boundary case, the final focused suite
  (`--lib -E 'test(session::syntax::tests)'`, same package/features): six passed.
- Scoped `cargo clippy --locked -j 2 -p merman-lsp --features stdio --all-targets -- -D warnings`
  passed, as did `cargo fmt --all -- --check` and `git diff --check`.

The full suite preceded only that last test addition; production code did not change afterward.
Shared parser/layout/render behavior is untouched by this LSP repair, so its admission gates are
LSP tests and scoped static checks rather than another SVG baseline sweep.
