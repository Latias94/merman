# ASCII Graph Parser Padding - Evidence And Gates

Status: Complete
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii flowchart`

## Broad Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29 AGP-010: Workstream opened with 7 target fixtures: 5 ASCII and 2 Unicode.
- 2026-05-29 AGP-020: Moved comment and explicit-label fixtures into exact parity, and fixed
  same-row reverse edges over self-loop lanes for preserve-order fixtures. Focused gates passed.
- 2026-05-29 AGP-030: Added `paddingX/Y` directive extraction for ASCII render entry points and
  moved custom padding/backlink short-Y fixtures into exact parity. Exact graph fixture parity moved
  from 53 to 60: 37 ASCII and 23 Unicode.
- 2026-05-29 AGP-040: Broad verification passed: `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check`.
