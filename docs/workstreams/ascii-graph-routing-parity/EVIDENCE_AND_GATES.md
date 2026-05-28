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
- 2026-05-29 AGR-020: Added `crates/merman-ascii/tests/graph_fixture.rs` with an explicit
  allowlist and a named gap inventory covering all copied graph fixtures under `ascii` and
  `extended-chars`.
- 2026-05-29 AGR-020 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
- 2026-05-29 AGR-030: Replaced the LR linear node layout with a reference-style 3x3 grid
  placement for roots and child levels, added basic same-row, down, down-then-right, and
  right-then-up edge routing, and aligned widened-column node labels with upstream centering.
  Exact graph fixture matches increased from 13 to 31:
  - ASCII: 16 exact matches, 36 named gaps.
  - Unicode: 15 exact matches, 8 named gaps.
- 2026-05-29 AGR-030 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
  - PASS `cargo nextest run -p merman-ascii` (43 passed)
  - PASS `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- 2026-05-29 AGR-040: Added LR self-loop routing, self-loop canvas extents, self-loop draw order
  over same-row edges, and same-row right-to-left back-edge routing. Exact graph fixture matches
  increased from 31 to 37:
  - ASCII: 19 exact matches, 33 named gaps.
  - Unicode: 18 exact matches, 5 named gaps.
- 2026-05-29 AGR-040 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS `cargo nextest run -p merman-ascii` (43 passed)
  - PASS `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- 2026-05-29 AGR-050: Updated simple subgraph layout to use upstream-style title rows inside the
  group box, removed empty-subgraph layout offsets, and documented the remaining complex subgraph
  boundary. Exact graph fixture matches increased from 37 to 44:
  - ASCII: 26 exact matches, 26 named gaps.
  - Unicode: 18 exact matches, 5 named gaps.
- 2026-05-29 AGR-050 gates:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS `cargo nextest run -p merman-ascii` (43 passed)
  - PASS `cargo clippy -p merman-ascii --all-targets -- -D warnings`
