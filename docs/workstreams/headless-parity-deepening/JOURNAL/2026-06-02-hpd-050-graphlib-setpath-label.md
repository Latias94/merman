# HPD-050 - Graphlib setPath Label API

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source test:
  - `repo-ref/graphlib/test/graph-test.js` `setPath / can set a value for all of the edges`
- Implementation source:
  - `repo-ref/graphlib/lib/graph.js` `setPath(vs, value)`

## Outcome

- Added `Graph::set_path_with_label(nodes, label)` as the Rust mapping for Graphlib's
  `setPath(nodes, value)` behavior.
- The method creates each edge in the path with the same label and updates existing labels when the
  path is applied again.
- The `E: Clone` bound is scoped to this method only. Ordinary graph construction and layout graph
  mutation still do not require cloneable edge labels.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the source case maps to
  `set_path_with_label_sets_and_updates_all_path_edge_labels`.

## Boundary

This is a small public Graph API seam, not a broad Graphlib ergonomics port. It does not add JS
argument overloading, chainability claims, ID stringification, or unused algorithms.

## Verification

- Red:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  failed because `Graph::set_path_with_label(...)` did not exist.
- Green:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  passed with `1` test.
- Package guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `81` tests.
- Downstream guard:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
