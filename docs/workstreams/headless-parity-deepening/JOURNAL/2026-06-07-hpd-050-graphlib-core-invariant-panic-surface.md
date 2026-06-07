# HPD-050 - Graphlib Core Invariant Panic Surface

Date: 2026-06-07

## Context

The Graphlib `Graph` API has several source-backed public throw mappings, including the named-edge
guard for non-multigraph simple graphs. This slice intentionally leaves those public API-shape
decisions alone.

The remaining target here was narrower: internal `expect(...)` calls that assume Graph storage
indexes and adjacency caches are coherent after prior mutations. Those invariants should hold for
normal public use, but they do not need to crash the process if future internal drift violates them.

## Change

- `compact(...)` now skips a remapped compound parent if its `children_ix` slot is unexpectedly
  missing instead of panicking.
- Directed and undirected adjacency-cache ensure paths now return an empty cache fallback if the
  cache slot is unexpectedly absent after ensure.
- `set_edge_named(...)` now returns without inserting if endpoint node indexes are unexpectedly
  missing after endpoint creation.
- The source-backed simple-graph named-edge panic remains unchanged.

## Verification

- `cargo +1.95 fmt -p dugong-graphlib`
- `cargo +1.95 nextest run -p dugong-graphlib --test graph_core_test`
- `rg -n 'children_ix resized to node slots|directed adjacency cache should be present after ensure|undirected adjacency cache should be present after ensure|ensure_node should have inserted the endpoint node' crates/dugong-graphlib/src/graph/core.rs`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check
- `git diff --check`

## Notes

- No Graphlib JSON schema, Dagre reference adapter behavior, renderer output, fixture baseline, or
  Architecture/Class residual classification changed.
- Existing `graph_core_test` coverage remains the relevant normal-path guard for compaction,
  adjacency queries, edge insertion, named-edge behavior, and optional labels.
