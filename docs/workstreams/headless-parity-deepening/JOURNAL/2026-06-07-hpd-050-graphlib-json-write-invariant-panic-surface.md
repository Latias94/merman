# HPD-050 - Graphlib JSON Write Invariant Panic Surface

Date: 2026-06-07

## Context

The Graphlib JSON seam already mirrors upstream `graphlib.json.write/read` for the source-backed
round-trip cases. Its writer path still had internal invariant `expect(...)` calls after iterating
`node_ids()` and `edge_keys()`.

Those invariants should hold for normal public `Graph` use, but this is library code in a
release-boundary panic-surface cleanup. If a future graph-internal drift exposes an id/key without a
live label lookup, JSON serialization should fail as an ordinary error instead of panicking.

## Change

- Replaced the node and edge writer `expect(...)` calls in `json::write(...)` and
  `json::write_with_defaults(...)` with explicit `ok_or_else(...)` error conversion.
- Added small Graphlib JSON invariant error helpers that return `serde_json::Error` backed by
  `io::ErrorKind::InvalidData`.
- Kept the normal Graphlib JSON shape, option-label semantics, default-collapsing helper behavior,
  parent serialization, and edge-key serialization unchanged.

## Verification

- `cargo +1.95 fmt -p dugong-graphlib`
- `cargo +1.95 nextest run -p dugong-graphlib --test json_test`
- `rg -n 'node_ids\(\) should only yield live nodes|edge_keys\(\) should only yield live edges' crates/dugong-graphlib/src/json.rs`
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check
- `git diff --check`

## Notes

- No renderer output, fixture baseline, Dagre reference adapter schema, or stored Mermaid parity
  baseline was changed.
- The existing `json_test` suite remains the relevant schema guard because the new error branch
  represents graph-internal invariant drift rather than a public JSON input shape.
