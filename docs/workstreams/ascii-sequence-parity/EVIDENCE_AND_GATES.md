# ASCII Sequence Parity - Evidence And Gates

Status: Active
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

## Broad Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29 ASP-010: Opened sequence parity lane after graph fixture parity closed. Copied
  upstream sequence parity starts at 17 exact fixtures: 12 Unicode and 5 ASCII. First executable
  product slice is open-arrow sequence messages from typed `SequenceDiagramRenderModel` values.
- 2026-05-29 ASP-020: Added open-arrow sequence message support for typed message types `5`
  (`A->B`) and `6` (`A-->B`). Unicode output now keeps open arrows visually distinct from filled
  `->>`/`-->>` messages. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii sequence`, and
  `cargo nextest run -p merman-ascii sequence_golden`.
- 2026-05-29 ASP-040: Broad verification passed before commit. `cargo nextest run -p merman-ascii`,
  `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`, and
  `git diff --check` all passed.
