# ASCII Graph Label Lanes - TODO

Status: Complete
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

- [x] AGL-030 [owner=codex] [deps=AGL-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Add duplicate/bidirectional edge-label lane support and move exact label fixtures into the allowlist.
  Validation:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture`
  - PASS `cargo nextest run -p merman-ascii flowchart`
  Review: Use graph structure/path metadata, not fixture-name special cases.
  Evidence: 5 graph fixtures moved to exact allowlist; `JOURNAL/2026-05-29-agl-030.md`
  Handoff: AGL-040 is next.

## M3 - Closeout

- [x] AGL-040 [owner=codex] [deps=AGL-030] [scope=docs/workstreams/ascii-graph-label-lanes,CHANGELOG.md,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Record final evidence, verify, commit, and close or split follow-ons.
  Validation:
  - PASS `cargo nextest run -p merman-ascii`
  - PASS `cargo nextest run -p merman --features ascii`
  - PASS `cargo nextest run -p merman-cli --features ascii`
  - PASS `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - PASS `git diff --check`
  Review: Remaining label or routing gaps are named.
  Evidence: `EVIDENCE_AND_GATES.md`, `JOURNAL/2026-05-29-agl-040.md`
  Handoff: Lane closed.
