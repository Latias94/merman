# HPD-050 - Graphlib Named-Edge Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the layout-invariant cleanup, the filtered production panic scan had one non-generated
runtime hit outside `merman-core` static JSON loading:

- `dugong-graphlib::Graph::set_edge_named(...)` panicked when callers attempted to set a named edge
  on a non-multigraph simple graph.

Upstream Graphlib throws for this JS API misuse. In Rust library code, the release-boundary policy
prefers a fallible API or graceful no-op over a public panic surface reachable by arbitrary caller
data.

## Changes

- Added `GraphError::NamedEdgeInNonMultigraph` and exported it from `dugong_graphlib`.
- Added `Graph::try_set_edge_named(...)` and `Graph::try_set_edge_key(...)` so callers can detect
  the source-backed named-edge/simple-graph violation explicitly.
- Kept the existing chainable `set_edge_named(...)` and `set_edge_key(...)` APIs, but changed this
  invalid named-edge case to a no-op instead of a panic.
- Updated Graphlib JSON reads to call `try_set_edge_named(...)`, so malformed simple-graph JSON
  with named edges returns `serde_json::Error` instead of silently dropping the edge.

## Verification

- `cargo +1.95 fmt -p dugong-graphlib` - passed.
- `cargo +1.95 nextest run -p dugong-graphlib --test graph_core_test --test json_test` - passed,
  `74` tests run.
- Filtered production scan across `merman-core`, `merman-render`, `dugong`, `dugong-graphlib`, and
  `manatee`, excluding tests, same-file `#[cfg(test)]` blocks, and comments, reports only the
  generated/static `merman-core` JSON validity checks:
  - `crates/merman-core/src/theme.rs:324`
  - `crates/merman-core/src/theme.rs:327`
  - `crates/merman-core/src/generated/mod.rs:13`

## Boundary

This is a Graphlib public API panic-surface cleanup. It does not change multigraph named-edge
storage, simple unnamed-edge behavior, edge lookup semantics, endpoint canonicalization, JSON
serialization format, Dugong layout geometry, Graphlib algorithm behavior, SVG baselines, root
viewport formulas, or Mermaid parity residual classification.
