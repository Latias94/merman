# Lint Allow Audit

This audit tracks source-level lint allowances that remain after the fearless-refactor cleanup.
Generated modules are excluded unless the allowance is placed in hand-written source that imports
generated code.

Last updated: 2026-05-09.

## Current Allowances

| Location | Lint | Status | Removal path |
| --- | --- | --- | --- |
| `crates/merman-core/src/diagrams/state/mod.rs` | `clippy::filter_map_identity` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `state_grammar.rs` at `items.into_iter().filter_map(|i| i).collect()`. | Regenerate or patch the grammar output so the generated parser emits `.flatten()` instead of `filter_map(identity)`, then remove the wrapper allowance. |
| `crates/merman-core/src/diagrams/flowchart.rs` | `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `flowchart_grammar.rs` with `result_large_err` around the parser return type and `type_complexity` around generated link segment tuple helpers. | Audit only after changing the generated parser/error shape; do not treat this as local hand-written cleanup. |
| `crates/merman-core/src/diagrams/sequence/mod.rs` | `clippy::type_complexity`, `clippy::result_large_err` | Required for generated LALRPOP parser code. Removing it makes `cargo clippy -p merman-core --all-targets --all-features -- -D warnings` fail in generated `sequence_grammar.rs` with `type_complexity` around generated participant tuple helpers. | Audit only after changing the generated parser/error shape; do not treat this as local hand-written cleanup. |
| `crates/merman-render/src/model.rs` | `clippy::large_enum_variant` | Structural layout API issue on `LayoutDiagram`. The enum is public, serialized, and widely matched by render dispatch, tests, and xtask tooling. | Design a boxed layout enum migration with serde compatibility checks and layout snapshot evidence before removing. |
| `crates/merman-core/src/diagrams/state/ast.rs` | `clippy::large_enum_variant` | Structural AST issue on `state::Stmt`; parser and state DB semantic application both consume the shape. | Box large AST variants only with parser snapshot coverage and state DOM parity checks. |

## Recently Removed

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
