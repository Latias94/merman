# HPD-050 Graphlib Graph Core Coverage

Date: 2026-06-02

## What Changed

- Added `crates/dugong-graphlib/tests/graph_core_test.rs` with the first direct source-test slice
  from `repo-ref/graphlib/test/graph-test.js`.
- Covered current public Rust API equivalents for:
  - graph initial options and graph label
  - node insertion, defaults, idempotence, and source queries
  - edge insertion, default labels, updates, named multiedges, path edges, and edge-key lookup
  - compound parent/children moves, clearing, root children, and remove-node cleanup
- Tightened compound parent assignment so setting a node under its own descendant panics with
  `set_parent would create a cycle`. Upstream Graphlib throws for this tree-invariant violation.

## Findings

- The source-backed tree invariant was a real missing guard, not a renderer-specific parity tweak.
  It is now enforced in `set_parent_ix(...)`, which is the shared parent-assignment path.
- The invalid non-compound `setParent(...)` behavior remains a Rust API-shape difference for now:
  upstream throws, while current Rust methods return `self` for non-compound graphs. This should be
  revisited deliberately before changing public API behavior.
- Graphlib JSON remains undecided. Avoid creating another serializer seam until `test/json-test.js`
  has a clear Rust API target.

## Verification

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
