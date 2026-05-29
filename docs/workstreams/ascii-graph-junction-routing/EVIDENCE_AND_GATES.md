# ASCII Graph Junction Routing - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii flowchart`

## Broader Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29 AGJ-010: Workstream opened as a follow-up to
  `docs/workstreams/ascii-graph-routing-parity`. The lane starts from 44 exact graph fixture
  matches and targets graph module split plus junction-aware LR routing.
- 2026-05-29 AGJ-020: Split `crates/merman-ascii/src/graph/mod.rs` into private renderer modules:
  `charset.rs`, `layout.rs`, `draw.rs`, and `routing.rs`. The split is intended to preserve
  behavior while making junction-aware route drawing local to `routing.rs`.
- 2026-05-29 AGJ-020 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
