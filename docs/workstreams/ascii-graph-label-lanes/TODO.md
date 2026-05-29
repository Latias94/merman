# ASCII Graph Label Lanes - TODO

Status: Active
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

- [x] AGL-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-graph-label-lanes]
  Goal: Open the label-lane follow-up and define the refactor/fixture boundary.
  Validation: Workstream docs exist and agree.
  Evidence: `DESIGN.md`
  Handoff: AGL-020 is next.

## M1 - Routing Module Deepening

- [x] AGL-020 [owner=codex] [deps=AGL-010] [scope=crates/merman-ascii/src/graph/routing.rs,crates/merman-ascii/src/graph/routing]
  Goal: Split path planning, path drawing, and labels out of the large routing module without intentional behavior changes.
  Validation:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture`
  - PASS `cargo nextest run -p merman-ascii graph::`
  Review: Preserve current 48 exact fixture matches.
  Evidence: `routing/path.rs`, `JOURNAL/2026-05-29-agl-020.md`
  Handoff: AGL-030 is next.

## M2 - Label Lane Parity

- [ ] AGL-030 [owner=codex] [deps=AGL-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Add duplicate/bidirectional edge-label lane support and move exact label fixtures into the allowlist.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Review: Use graph structure/path metadata, not fixture-name special cases.
  Evidence: Allowlist/gap delta and task journal.
  Handoff: AGL-040 is next.

## M3 - Closeout

- [ ] AGL-040 [owner=codex] [deps=AGL-030] [scope=docs/workstreams/ascii-graph-label-lanes,CHANGELOG.md,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Record final evidence, verify, commit, and close or split follow-ons.
  Validation:
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: Remaining label or routing gaps are named.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: Lane closes or hands off with concrete next task.
