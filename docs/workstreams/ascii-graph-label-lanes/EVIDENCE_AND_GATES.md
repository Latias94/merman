# ASCII Graph Label Lanes - Evidence And Gates

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

- 2026-05-29 AGL-010: Workstream opened from the closed junction-routing follow-up. Starting graph
  fixture parity is 48 exact matches: 28 ASCII and 20 Unicode.
- 2026-05-29 AGL-020: Split grid path planning into `graph/routing/path.rs` with no intentional
  behavior change. Gates passed: `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii graph_fixture`, and
  `cargo nextest run -p merman-ascii graph::`.
