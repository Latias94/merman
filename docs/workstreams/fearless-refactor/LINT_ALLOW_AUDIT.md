# Lint Allow Audit

This audit tracks source-level lint allowances that remain after the fearless-refactor cleanup.
Generated modules are excluded unless the allowance is placed in hand-written source that imports
generated code. The primary release surface is `merman-core`, `merman-render`, and `xtask`; the
workspace support crates are tracked separately below because they are still covered by the
workspace clippy gate.

Last updated: 2026-05-11.

## Mainline Allowances

| Location | Lint | Status | Removal path |
| --- | --- | --- | --- |
| `crates/merman-core/src/diagrams/class/mod.rs` | `clippy::empty_line_after_outer_attr` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `class_grammar.rs` around generated `__ToTriple` helpers. | Remove only after LALRPOP output no longer emits blank lines after outer attributes. |
| `crates/merman-core/src/diagrams/er.rs` | `clippy::empty_line_after_outer_attr` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `er_grammar.rs` around generated `__ToTriple` helpers. | Remove only after LALRPOP output no longer emits blank lines after outer attributes. |
| `crates/merman-core/src/diagrams/state/mod.rs` | `clippy::empty_line_after_outer_attr`, `clippy::filter_map_identity` | Required for generated LALRPOP parser code. Removing these makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `state_grammar.rs` around generated `___ToTriple` helpers and `items.into_iter().filter_map(|i| i).collect()`. | Regenerate or patch the grammar output so the generated parser emits `.flatten()` and no blank line after generated outer attributes, then remove the wrapper allowances. |
| `crates/merman-core/src/diagrams/flowchart.rs` | `clippy::empty_line_after_outer_attr`, `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `flowchart_grammar.rs` with empty-line output around generated tuple helpers, `result_large_err` around the parser return type, and `type_complexity` around generated link segment tuple helpers. | Audit only after changing the generated parser/error/output shape; do not treat this as local hand-written cleanup. |
| `crates/merman-core/src/diagrams/sequence/mod.rs` | `clippy::empty_line_after_outer_attr`, `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `sequence_grammar.rs` with empty-line output around generated tuple helpers and `type_complexity` around generated participant tuple helpers. | Audit only after changing the generated parser/error/output shape; do not treat this as local hand-written cleanup. |
| `crates/merman-render/src/trig_tables.rs` | `clippy::excessive_precision` | Required for Node/V8-generated trig constants used by strict Flowchart stadium arc point parity. The literals intentionally keep the source measurement precision, and the module is `#[rustfmt::skip]` to preserve table readability. | Regenerate the table from the documented Node/V8 source if the upstream point generation changes; do not trim precision only to satisfy clippy because strict `data-points` parity is sensitive at tiny deltas. |

## Workspace Support Crate Allowances

These are not generated parser wrappers, but they live in parity-oriented support crates that
mirror upstream layout or RoughJS algorithms. They should stay visible in this audit even when the
mainline renderer/core surface is clean.

| Location | Lint | Status | Removal path |
| --- | --- | --- | --- |
| `crates/manatee/src/algo/fcose/mod.rs` | `dead_code`, `clippy::collapsible_if`, `clippy::manual_div_ceil`, `clippy::needless_option_as_deref`, `clippy::needless_range_loop`, `clippy::nonminimal_bool` | Retained for the FCoSE port while it stays close to the upstream Cytoscape/FCoSE control flow and debug surface. Redundant item-level `dead_code` allowances inside this module were removed after the module-level allowance was confirmed to cover them. | Split the FCoSE port into smaller owner modules, delete unused debug/reference helpers, and then remove these allowances under `cargo clippy -p manatee --all-targets --all-features -- -D warnings`. |
| `crates/manatee/src/algo/fcose/spectral.rs` | `clippy::assign_op_pattern`, `clippy::manual_contains`, `clippy::manual_swap`, `clippy::needless_range_loop` | Retained for the spectral initialization port where loop shape and operation order are still intentionally close to upstream `cytoscape-fcose`. | Refactor only with same-machine Architecture/Mindmap parity and timing evidence, then remove under the `manatee` clippy gate. |
| `crates/roughr/src/renderer.rs`, `crates/roughr/src/generator.rs` | `clippy::too_many_arguments` | Retained only for the public `arc` entrypoints in the forked RoughJS API shape. The private ellipse/arc/bezier helper signatures have been moved behind small request structs. | Add public arc request structs or accept the public compatibility shape after Flowchart/State hand-drawn parity and allocation checks stay green. |

## Generated Exclusions

- None. `crates/merman-render/src/generated/mod.rs` no longer keeps a blanket
  `#![allow(clippy::all)]`; the generated and fixture-derived parity data now passes
  `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` without a module
  umbrella allowance.

## Recently Removed

- `crates/merman-core/src/lib.rs`: removed the crate-level `clippy::empty_line_after_outer_attr`
  allowance by scoping it to the individual LALRPOP wrapper modules that include generated parser
  output with blank lines after outer attributes.
- `crates/merman-render/src/model.rs`: removed `clippy::large_enum_variant` from public
  `LayoutDiagram` by boxing diagram layout payloads while preserving serde layout shape under
  layout snapshot and SVG parity checks.
- `crates/merman-core/src/diagrams/state/ast.rs`: removed `clippy::large_enum_variant` from
  `state::Stmt` by moving relation payloads behind a boxed `RelationStmt`, preserving state parser
  behavior under State tests and SVG DOM parity checks.
- `crates/merman-core/src/diagrams/flowchart/ast.rs`: removed `clippy::large_enum_variant` from
  `flowchart::Stmt` by boxing the standalone node statement variant and keeping the parser/build
  path behavior-equivalent under Flowchart tests and SVG DOM parity checks.
- `crates/xtask/src/cmd/overrides/font_metrics.rs`: removed `clippy::needless_range_loop` from the
  font-metrics ridge solver by making the solver module-local and covering ordinary and pivoting
  systems with focused unit tests.
- `crates/merman-render/src/generated/mod.rs`: removed the module-level `clippy::all` allowance
  after replacing the generated font-metrics lookup loop with `Iterator::find` and updating the
  `xtask gen-font-metrics` template to emit the clippy-clean form; generated and fixture-derived
  parity data now stays under normal clippy coverage.
- `crates/manatee/src/algo/fcose/mod.rs`: removed redundant item-level `dead_code` allowances from
  fields, a constant, and an RNG helper that were already covered by the module-level allowance.
- `crates/dugong/src/position/bk/util.rs` and `crates/dugong/src/position/bk/core.rs`: removed
  the unused private BK helper `edge_key` and the stale reference-only `vertical_alignment_ref`
  implementation, clearing the last `dead_code` allowances in that subtree.
- `crates/roughr/src/core.rs`, `crates/roughr/src/generator.rs`, and
  `crates/roughr/src/filler/scan_line_hachure.rs`: removed the unused `Space` / `Config` /
  `DrawingSurface` shells, the dead `Generator::new` constructor, and the unused
  `ActiveEdgeEntry.s` field, clearing the `roughr` dead-code allowance bucket.
- `crates/roughr/src/renderer.rs`: moved the private `_compute_ellipse_points`, `_arc`, and
  `_bezier_to` argument bundles behind small request structs, leaving only the public `arc`
  compatibility entrypoints with `clippy::too_many_arguments` allowances.

## Gate

Use the release gate before landing lint-allow cleanup:

```sh
cargo run -p xtask -- verify --strict
```

Use a narrower first pass while iterating:

```sh
cargo clippy -p merman-core -p merman-render -p xtask --all-targets --all-features -- -D warnings
```
