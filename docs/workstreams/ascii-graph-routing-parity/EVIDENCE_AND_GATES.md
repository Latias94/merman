# ASCII Graph Routing Parity - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii graph_fixture`

## Broader Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29: Workstream opened after `ascii-renderer-compatibility-expansion` was committed as
  `da528d3d feat: expand ascii flowchart compatibility`.
- 2026-05-29 AGR-010: Split ASCII graph model and flowchart adapter into
  `crates/merman-ascii/src/graph/model.rs` and `crates/merman-ascii/src/graph/adapter.rs`.
  `merman-core` flowchart types are now isolated to the adapter boundary.
- 2026-05-29 AGR-010 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 19 skipped)
  - PASS `cargo nextest run -p merman-ascii graph::` (6 passed, 35 skipped)
