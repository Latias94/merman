# HPD-050 - Graphlib Stringified-ID Boundary

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The Graphlib public API coverage ledger still had upstream `graph-test.js` stringification cases
that were not clearly resolved. These cases exist because JS Graphlib accepts dynamic ids such as
numbers and coerces them through string conversion before storing node ids, parent ids, edge
endpoints, and edge names.

Rust `dugong-graphlib` intentionally exposes string-shaped graph ids through `impl Into<String>`
setters and `&str` lookups, so the same arbitrary JS argument coercion is not a direct Rust API
target. The useful consumer question is narrower: once endpoints are stringified, do undirected
edge keys follow the same string-order canonicalization rule as upstream?

## Source Finding

Pinned `repo-ref/graphlib/lib/graph.js::edgeArgsToId(...)` does:

- `var v = "" + v_`
- `var w = "" + w_`
- if the graph is undirected and `v > w`, swaps `v` and `w`

Therefore Graphlib's numeric sample `setEdge(9, 10, "foo")` is stored using string comparison, so
the canonical endpoint order is `"10"` then `"9"`, not numeric order.

## Outcome

- Added `undirected_edges_follow_graphlib_string_order_for_stringified_ids` in
  `crates/dugong-graphlib/tests/graph_core_test.rs`.
- The test uses already-stringified endpoints `"9"` and `"10"`, verifies lookup works in both
  directions, and asserts the stored edge key is canonicalized to `"10"` / `"9"`.
- Updated `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md` so the covered post-coercion
  undirected ordering case points at the new Rust regression.
- Documented numeric/object JS id coercion as a Rust API-shape non-target unless a future FFI/raw
  Graphlib input bridge accepts arbitrary dynamic ids.
- No production Graphlib or renderer behavior changed.

## Verification

- `cargo nextest run -p dugong-graphlib undirected_edges_follow_graphlib_string_order_for_stringified_ids` -
  passed, `1` test run.
- `cargo nextest run -p dugong-graphlib` - passed, `97` tests run.
- `cargo fmt --check -p dugong-graphlib` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF-to-CRLF warning only.
- JSON parse gates passed for `CONTEXT.jsonl` (`562` records) and `WORKSTREAM.json`.

## Residual Boundary

Do not add JS-like numeric/object id coercion to the typed Rust Graph API for parity optics. Reopen
that only if a concrete FFI or raw Graphlib input surface starts accepting untyped ids from hosts.
