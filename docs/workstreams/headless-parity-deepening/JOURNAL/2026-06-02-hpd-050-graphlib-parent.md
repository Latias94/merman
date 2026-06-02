# HPD-050 - Graphlib Parent Query Coverage

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source tests:
  - `repo-ref/graphlib/test/graph-test.js` `parent / returns undefined if the graph is not compound`
  - `repo-ref/graphlib/test/graph-test.js` `parent / returns undefined if the node is not in the graph`
  - `repo-ref/graphlib/test/graph-test.js` `parent / defaults to undefined for new nodes`
  - `repo-ref/graphlib/test/graph-test.js` `parent / returns the current parent assignment`
  - `repo-ref/graphlib/test/graph-test.js` `setParent` clear-parent cases

## Outcome

- Added `parent_matches_graphlib_optional_query_shape` to lock the Rust `parent(...) -> Option<&str>`
  mapping for Graphlib's `parent(v)` optional query behavior.
- Extended `clear_parent_returns_node_to_root_children` so Graphlib's `setParent(v)` /
  `setParent(v, undefined)` state behavior maps to Rust's explicit `clear_parent(v)` API and remains
  idempotent.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the source cases no longer remain
  implicit.

## Boundary

No production code changed in this slice. The current Rust APIs already match the state behavior.
JS optional-argument overloading, chainability, non-compound `setParent(...)` throws, and ID
stringification remain explicit API-shape differences.

## Verification

- Focused guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib parent`
  passed with `9` tests.
- Format:
  `cargo fmt --check -p dugong-graphlib`
  passed.
- Package guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `84` tests.
- Downstream guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
