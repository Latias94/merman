# HPD-050 - Graphlib setEdge Optional Labels And EdgeKey

Date: 2026-06-02
Task: HPD-050 layout engine / Dugong-adjacent source audit

## Source Evidence

- Pinned Graphlib source: `repo-ref/graphlib` at
  `380d5efa1f4ab0904539f046bdba583d14ac2add`.
- Source tests:
  - `repo-ref/graphlib/test/graph-test.js` `setEdge / deletes the value for the edge if the value arg is undefined`
  - `repo-ref/graphlib/test/graph-test.js` `setEdge / changes the value for a multi-edge if it is already in the graph`
  - `repo-ref/graphlib/test/graph-test.js` `setEdge / can take an edge object as the first parameter`
  - `repo-ref/graphlib/test/graph-test.js` `setEdge / can take an multi-edge object as the first parameter`

## Outcome

- Added regressions showing Rust maps explicit JS `undefined` edge-label clearing through
  `Option<T>` edge labels: `Some(None)` clears an existing optional label while preserving the edge.
- Added `set_edge_key_sets_simple_and_named_edge_labels` so Graphlib edge-object parameters map to
  Rust `EdgeKey`.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the source cases no longer remain
  implicit.

## Boundary

No production code changed in this slice. JS argument overloading, chainability, and ID
stringification remain explicit API-shape differences; Rust callers use typed `Option<T>` labels
and `EdgeKey`.

## Verification

- Focused red:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  failed first on an unnamed `EdgeKey::new(..., None)` type-inference issue in the new regression.
- Focused green:
  `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  passed after spelling the unnamed edge-object key as `None::<String>`.
- `cargo fmt --check -p dugong-graphlib` passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `87` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
- JSONL validation passed for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`.
- `git diff --check` passed.
