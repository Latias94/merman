# ASCII Renderer Compatibility Expansion - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Policy

Record fresh command evidence when completing each task. Use focused gates while implementing and
broader gates before closeout.

## Focused Gates

- `cargo nextest run -p merman-ascii flowchart`
- `cargo nextest run -p merman-ascii graph::`
- `cargo check -p merman-ascii`

## CLI Gates

- `cargo nextest run -p merman-cli --features ascii ascii`
- `cargo check -p merman-cli --features ascii`

## Closeout Gates

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings`
- `git diff --check`

## Evidence Log

- 2026-05-28: Workstream opened after `ascii-renderer-productization` closed. No implementation
  evidence recorded yet.
- 2026-05-28: ACE-010 completed.
  - `DESIGN.md` records the terminal approximation policy.
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md` now lists V1.1 planned behavior for edge labels,
    open/dotted/length-modified edges, common node-shape approximations, and subgraphs.
  - No Rust validation was required because ACE-010 is a documentation and compatibility-policy
    task.
- 2026-05-28: ACE-020 completed.
  - Added internal flowchart edge metadata for labels, stroke style, arrow kind, and requested
    length.
  - Added parser/model coverage for LR and TB edge labels, dotted edges, open edges, length
    modifiers, and unsupported thick/cross variants.
  - `cargo nextest run -p merman-ascii flowchart` passed: 20 tests.
  - `cargo nextest run -p merman-ascii graph::` passed: 6 tests.
  - `cargo check -p merman-ascii` passed.
- 2026-05-28: ACE-030 completed.
  - Added internal flowchart node-shape metadata and terminal approximations for rounded/circle,
    diamond/decision, subroutine, and cylinder/database shapes.
  - Added parser coverage for circle, diamond, subroutine, and cylinder syntax.
  - Existing graph golden tests remained stable for rectangular nodes.
  - `cargo nextest run -p merman-ascii flowchart` passed: 22 tests.
  - `cargo nextest run -p merman-ascii graph::` passed: 6 tests.
  - `cargo check -p merman-ascii` passed.
- 2026-05-28: ACE-040 completed.
  - Added simple group metadata and group-box layout for flowchart subgraphs.
  - Added parser/model coverage for titled subgraph output.
  - Existing graph golden tests remained stable for diagrams without subgraphs.
  - `cargo nextest run -p merman-ascii flowchart` passed: 22 tests.
  - `cargo nextest run -p merman-ascii graph::` passed: 6 tests.
  - `cargo check -p merman-ascii` passed.
- 2026-05-28: ACE-050 completed.
  - Updated root README, `crates/merman-ascii/README.md`, and `CHANGELOG.md`.
  - Updated CLI ASCII smoke coverage to render a flowchart with a subgraph, edge label, round
    shape, and cylinder/database shape through `merman-cli render --format ascii`.
  - `cargo nextest run -p merman-cli --features ascii ascii` passed: 2 tests.
  - `cargo check -p merman-cli --features ascii` passed.
- 2026-05-28: ACE-060 closeout completed.
  - `cargo fmt --all --check` passed.
  - `cargo nextest run -p merman-ascii` passed: 41 tests.
  - `cargo nextest run -p merman --features ascii` passed: 3 tests.
  - `cargo nextest run -p merman-cli --features ascii` passed: 10 tests.
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
    passed.
  - `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings` passed.
  - `git diff --check` passed.
