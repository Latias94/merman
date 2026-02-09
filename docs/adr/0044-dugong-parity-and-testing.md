# ADR 0044: Dugong Parity Baseline and Testing Strategy

## Status

Accepted

## Context

`merman` is a headless, pure-Rust reimplementation of Mermaid `@11.12.2`. Achieving rendering parity
for DAG-based diagrams requires a Dagre-class layout engine plus a Graphlib-like graph container.

We will implement Dagre in Rust as a general-purpose library named `dugong`, with the graph container
extracted as `dugong-graphlib`.

To avoid subjective drift, we need:

- a pinned upstream baseline (tag + commit) for dagre,
- a mechanical parity loop (upstream tests → Rust tests),
- documented coverage that is easy to extend incrementally.

## Decision

### Baseline

- `dugong` baseline is `repo-ref/dagre` pinned to:
  - package: `@dagrejs/dagre@2.0.2`
  - commit: `ba986662394f8f3ed608717194e5958f3386ce01`
- `dugong-graphlib` baseline is `@dagrejs/graphlib@2.2.4` (required by Dagre).
  - commit: `380d5efa1f4ab0904539f046bdba583d14ac2add`
  - if a local checkout is used, it lives under `repo-ref/graphlib` and is pinned in
    `repo-ref/REPOS.lock.json` (not a git submodule).

### Public API compatibility

- `dugong-graphlib` aims to be source-compatible (conceptually) with upstream Graphlib’s Graph API:
  - compound graphs (`parent/children`)
  - multigraphs (edge `name` keys)
  - graph-level attributes (`setGraph/getGraph` equivalent)
  - default node/edge labels (`setDefaultNodeLabel/setDefaultEdgeLabel` equivalent)
- `dugong` exposes a Dagre-style entrypoint:
  - `layout(graph)` mutates the graph by setting node positions and edge routes (points), matching
    upstream semantics.
  - `layout_dagreish(graph)` provides a parity-oriented pipeline that mirrors Dagre’s layout
    sequence more closely (rank/normalize/order/BK positioning), and is used by `merman` where SVG
    parity requires Dagre-compatible behavior.
    - It is gated behind the `dugong/dagreish` feature (enabled by default) to keep the minimal
      pipeline lightweight for downstream consumers that do not need parity mode.

### Dagre pipeline notes (connectivity + compound graphs)

Upstream Dagre runs ranking on a **non-compound view** of the graph:

- `nestingGraph.run(g)`
- `rank(asNonCompoundGraph(g))`

This is important because cluster nodes (nodes with children) do not participate in ranking and
would otherwise break network-simplex connectivity assumptions.

### Testing strategy (primary)

Port upstream Dagre Jest tests directly into Rust tests, one file at a time:

- Upstream: `repo-ref/dagre/test/*.js`
- Rust: `crates/dugong/tests/*` and `crates/dugong-graphlib/tests/*`

Porting rules:

- keep test case intent and inputs identical (node sizes, graph options, edges, parents, weights),
- assert on the same observable outputs (node `x/y`, edge `points`, edge label `x/y`, etc.),
- prefer small, deterministic unit tests over large snapshot blobs for early bring-up.

Coverage tracking:

- maintain `docs/dugong/DAGRE_UPSTREAM_TEST_COVERAGE.md` mapping each upstream test file and `it(...)`
  title to a Rust test function name.
- maintain `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` only if we end up porting Graphlib tests
  independently (otherwise coverage is implicit via Dagre tests).

### Numeric determinism

Layout computations use floating-point arithmetic and can be sensitive to ordering.

Rules:

- use `f64` internally for coordinates and intermediate calculations.
- define deterministic iteration order when traversing nodes/edges:
  - never rely on hash map iteration; sort keys where order affects results.
- for tests, allow tolerance where upstream uses floating computations:
  - use a small epsilon comparison for floats (exact values are required only when upstream’s
    results are provably integral or directly derived from inputs).

### Out of scope for dugong

`dugong` does not implement:

- text measurement, font shaping, or HTML label layout,
- SVG generation or DOM manipulation,
- Mermaid-specific cluster/subgraph post-processing (those live in `merman` rendering layer).

Those concerns are handled by `merman` via a headless rendering pipeline and pluggable measurers.

## Consequences

- `dugong` becomes a reusable Rust layout engine with a clear, test-driven parity baseline.
- `merman` can depend on `dugong` for layout while keeping rendering backends and UI integration
  separate and headless.
- Exact Mermaid SVG parity remains dependent on faithful porting of Mermaid-specific rendering
  semantics and deterministic text measurement.
