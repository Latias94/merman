# ASCII Graph Label Lanes - Evidence And Gates

Status: Complete
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
- 2026-05-29 AGL-030: Added routed edge label collection/overlay, LR parallel bottom lanes, TD
  right-side back lanes, and label-driven graph spacing. Exact graph fixture parity moved from 48
  to 53: 32 ASCII and 21 Unicode. Gates passed: `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii graph_fixture`,
  `cargo nextest run -p merman-ascii graph::`, and
  `cargo nextest run -p merman-ascii flowchart`.
- 2026-05-29 AGL-040: Broad verification passed: `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check`.
