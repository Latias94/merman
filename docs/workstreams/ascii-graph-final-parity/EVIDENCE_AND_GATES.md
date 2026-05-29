# ASCII Graph Final Parity - Evidence And Gates

Status: Complete
Last updated: 2026-05-29

## Focused Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii graph_fixture`
- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii flowchart`

## Broad Gates

- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-29 AGF-010: Workstream opened from the remaining graph gaps. Starting graph fixture
  parity is 60 exact matches: 37 ASCII and 23 Unicode.
- 2026-05-29 AGF-020: Split graph routing internals into route-cell and routed-label modules
  without intentional behavior changes. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii graph_fixture`, `cargo nextest run -p merman-ascii graph::`,
  and `git diff --check`.
- 2026-05-29 AGF-030: Added line-aware graph node labels with `<br>` and escaped newline
  normalization. Moved `ascii/multiline_single_node.txt` into exact parity. Current graph fixture
  parity is 61 exact matches: 38 ASCII and 23 Unicode. Passed `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii graph::label`,
  `cargo nextest run -p merman-ascii graph_fixture`, `cargo nextest run -p merman-ascii flowchart`,
  and `git diff --check`.
- 2026-05-29 AGF-040: Added subgraph parity rules for dynamic group offset, TD branch layout,
  external incoming subgraph padding, final subgraph-label overlay, LR external/subgraph root
  separation, and nested group bounds. Moved every remaining ASCII subgraph fixture into exact
  parity. Current graph fixture parity is 75 exact matches: 52 ASCII and 23 Unicode. Passed
  `cargo fmt --all --check`, `cargo nextest run -p merman-ascii graph_fixture`,
  `cargo nextest run -p merman-ascii flowchart`, and `git diff --check`.
- 2026-05-29 AGF-050: Broad closeout passed. `cargo fmt --all --check`,
  `cargo nextest run -p merman-ascii`, `cargo nextest run -p merman --features ascii`,
  `cargo nextest run -p merman-cli --features ascii`,
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`,
  and `git diff --check` all passed before closeout docs were finalized.
