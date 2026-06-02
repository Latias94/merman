# HPD-050 Graphlib Edge Invariants

Date: 2026-06-02

## What Changed

- Tightened simple-graph named-edge behavior in `dugong-graphlib`:
  - setting a named edge on a non-multigraph now panics,
  - named-edge queries no longer match an unnamed edge in a non-multigraph, and
  - removing a named edge from a non-multigraph no longer removes the unnamed edge.
- Added direct `repo-ref/graphlib/test/graph-test.js` coverage for edge-key listing, directed vs.
  undirected edge lookup, missing edge lookup, named-edge rejection, named edge removal, and
  undirected remove-edge endpoint normalization.
- Updated the Graphlib coverage ledger so this invariant is recorded as a real source-backed
  behavior fix rather than a renderer-specific tweak.

## Findings

- The previous Rust behavior silently discarded a supplied edge name when `multigraph = false`.
  Upstream Graphlib throws in that case. That difference could hide accidental caller bugs because
  a named edge lookup or remove could target the unnamed edge.
- Production Mermaid-facing renderers that use named edges already construct multigraphs, so the
  stricter invariant matches the intended graph model instead of forcing a renderer rewrite.
- JS id stringification remains out of scope for the Rust core API. This is documented separately
  because it is a language-boundary behavior, not a layout invariant.

## Verification

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `cargo test -p merman-render --test flowchart_layout_test`
- `cargo test -p merman-render --test state_layout_test`
- `cargo test -p merman-render --test class_layout_test`
- `cargo test -p merman-render --test er_layout_test`
