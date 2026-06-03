# HPD-050 - Dagre Reference Graphlib JSON Consumer

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## What Changed

- Updated [crates/xtask/src/cmd/debug/dagre_reference.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre_reference.rs)
  so Dagre reference input and Rust output artifacts are serialized through
  `dugong::graphlib::json` instead of a separate `graph` / `id` / `label` debug JSON shape.
- Updated [tools/dagre-harness/run.mjs](/F:/SourceCodes/Rust/merman/tools/dagre-harness/run.mjs)
  to accept the new Graphlib JSON shape and to write JS output through installed
  `dagre-d3-es` Graphlib `json.write(...)`.
- Kept the harness backward compatible with older debug inputs while making new artifacts use
  Graphlib JSON `options`, top-level `value`, node `v` / `value`, edge `v` / `w` / `name` /
  `value`, and optional `parent`.

## Findings

- The Graphlib JSON seam is now connected to a real consumer: `compare-dagre-layout` reference
  artifacts. This removes an active ad hoc Graphlib-shaped serializer rather than adding unused API
  surface.
- Dagre labels remain projected explicitly inside xtask as JSON objects. The production Dagre label
  structs did not need broad serde derives.
- No renderer or layout behavior changed. The State `basic` JS/Rust Dagre comparison stayed
  zero-delta after the artifact shape changed.

## Verification

- `cargo nextest run -p xtask dagre_reference_input_uses_graphlib_json_shape` - failed before the
  adapter change, then passed after the Graphlib JSON consumer seam landed.
- `cargo nextest run -p xtask dagre_reference` - passed, `3` tests covering compound-edge
  normalization plus Graphlib JSON input and Rust output artifact shapes.
- `cargo run -p xtask -- compare-dagre-layout --diagram state --fixture basic --out-dir target\compare\dagre-layout-hpd050-graphlib-json` -
  passed with max node delta `0.000000` and max edge delta `0.000000`.
- `cargo nextest run -p dugong-graphlib --test json_test` - passed, `8` tests.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed on the post-merge worktree; implemented-matrix structural parity remains green.
