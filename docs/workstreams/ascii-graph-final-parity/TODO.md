# ASCII Graph Final Parity - TODO

Status: Active
Last updated: 2026-05-29

## M0 - Scope Freeze

- [x] AGF-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-graph-final-parity]
  Goal: Open the final graph parity lane and freeze remaining target fixtures.
  Validation: Workstream docs exist and agree.
  Evidence: `DESIGN.md`
  Handoff: AGF-020 is next.

## M1 - Routing Module Deepening

- [x] AGF-020 [owner=codex] [deps=AGF-010] [scope=crates/merman-ascii/src/graph/routing.rs,crates/merman-ascii/src/graph/routing]
  Goal: Split route-cell merging and label placement out of `routing.rs` without intentional
  behavior changes.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii graph::`
  Evidence: Module files and journal.
  Handoff: AGF-030 is next.

## M2 - Multiline Labels

- [ ] AGF-030 [owner=codex] [deps=AGF-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Support multiline node labels sufficiently to move `multiline_single_node` into exact
  parity.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Evidence: Fixture delta and tests.
  Handoff: AGF-040 is next.

## M3 - Subgraph Heavy Parity

- [ ] AGF-040 [owner=codex] [deps=AGF-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Move as many remaining subgraph-heavy fixtures as possible into exact parity without
  fixture-name special cases.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Evidence: Gap delta and journal.
  Handoff: AGF-050 is next.

## M4 - Closeout

- [ ] AGF-050 [owner=codex] [deps=AGF-040] [scope=docs/workstreams/ascii-graph-final-parity,CHANGELOG.md,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Run broad gates, update docs/gap inventory, commit, and close or split any irreducible
  follow-on.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: Lane closes or names concrete follow-on blockers.
