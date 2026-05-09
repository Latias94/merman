# Lint Allow Audit

This audit tracks source-level lint allowances that remain after the fearless-refactor cleanup.
Generated modules are excluded unless the allowance is placed in hand-written source that imports
generated code.

Last updated: 2026-05-09.

## Current Allowances

| Location | Lint | Status | Removal path |
| --- | --- | --- | --- |
| `crates/merman-core/src/diagrams/class/mod.rs` | `clippy::empty_line_after_outer_attr` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `class_grammar.rs` around generated `__ToTriple` helpers. | Remove only after LALRPOP output no longer emits blank lines after outer attributes. |
| `crates/merman-core/src/diagrams/er.rs` | `clippy::empty_line_after_outer_attr` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `er_grammar.rs` around generated `__ToTriple` helpers. | Remove only after LALRPOP output no longer emits blank lines after outer attributes. |
| `crates/merman-core/src/diagrams/state/mod.rs` | `clippy::empty_line_after_outer_attr`, `clippy::filter_map_identity` | Required for generated LALRPOP parser code. Removing these makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `state_grammar.rs` around generated `___ToTriple` helpers and `items.into_iter().filter_map(|i| i).collect()`. | Regenerate or patch the grammar output so the generated parser emits `.flatten()` and no blank line after generated outer attributes, then remove the wrapper allowances. |
| `crates/merman-core/src/diagrams/flowchart.rs` | `clippy::empty_line_after_outer_attr`, `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `flowchart_grammar.rs` with empty-line output around generated tuple helpers, `result_large_err` around the parser return type, and `type_complexity` around generated link segment tuple helpers. | Audit only after changing the generated parser/error/output shape; do not treat this as local hand-written cleanup. |
| `crates/merman-core/src/diagrams/sequence/mod.rs` | `clippy::empty_line_after_outer_attr`, `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `sequence_grammar.rs` with empty-line output around generated tuple helpers and `type_complexity` around generated participant tuple helpers. | Audit only after changing the generated parser/error/output shape; do not treat this as local hand-written cleanup. |

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

## Gate

Use the release gate before landing lint-allow cleanup:

```sh
cargo run -p xtask -- verify --strict
```

Use a narrower first pass while iterating:

```sh
cargo clippy -p merman-core -p merman-render -p xtask --all-targets --all-features -- -D warnings
```
