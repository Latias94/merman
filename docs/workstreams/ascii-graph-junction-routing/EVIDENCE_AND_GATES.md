# ASCII Graph Junction Routing - Evidence And Gates

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
- 2026-05-29 AGJ-030: Ported the high-value shape of `mermaid-ascii`'s Go routing core into the
  Rust graph renderer: 3x3 grid ports, A* path search, path segment merging, route-cell-only
  junction merging, Unicode box-start connectors, and corner/arrow drawing. Exact graph fixture
  matches increased from 44 to 48:
  - ASCII: 28 exact matches, 24 named gaps.
  - Unicode: 20 exact matches, 3 named gaps.
- 2026-05-29 AGJ-030 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
- 2026-05-29 AGJ-040: Closeout complete. `FLOWCHART_SUPPORT.md`, `CHANGELOG.md`, the fixture
  allowlist, and the gap inventory now reflect Go-style LR path routing and 48 exact graph fixture
  matches.
- 2026-05-29 AGJ-040 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii` (43 passed)
  - PASS `cargo nextest run -p merman --features ascii` (3 passed)
  - PASS `cargo nextest run -p merman-cli --features ascii` (10 passed)
  - PASS `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - PASS `git diff --check`
