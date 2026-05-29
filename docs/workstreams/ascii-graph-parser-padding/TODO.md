# ASCII Graph Parser Padding - TODO

Status: Complete
Last updated: 2026-05-29

## M0 - Scope Freeze

- [x] AGP-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-graph-parser-padding]
  Goal: Open the parser/order/padding lane and freeze target fixtures.
  Validation: Workstream docs exist and agree.
  Evidence: `DESIGN.md`
  Handoff: AGP-020 is next.

## M1 - Parser And Model Semantics

- [x] AGP-020 [owner=codex] [deps=AGP-010] [scope=crates/merman-core/src/diagrams/flowchart,crates/merman-ascii/src/graph/adapter.rs,crates/merman-ascii/tests]
  Goal: Make comments, declaration order, and explicit-label precedence match target fixtures.
  Validation:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture`
  - PASS `cargo nextest run -p merman-ascii flowchart`
  Evidence: `JOURNAL/2026-05-29-agp-020.md`
  Handoff: AGP-030 is next.

## M2 - Padding And Route Spacing

- [x] AGP-030 [owner=codex] [deps=AGP-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Support copied fixture padding directives and route-grid spacing needed by short-Y backlink
  layouts.
  Validation:
  - PASS `cargo fmt --all --check`
  - PASS `cargo nextest run -p merman-ascii graph_fixture`
  - PASS `cargo nextest run -p merman-ascii flowchart`
  - PASS `cargo nextest run -p merman --features ascii`
  Evidence: `custom_padding` and `backlink_with_short_y_padding` parity; `JOURNAL/2026-05-29-agp-030.md`
  Handoff: AGP-040 is next.

## M3 - Closeout

- [x] AGP-040 [owner=codex] [deps=AGP-030] [scope=docs/workstreams/ascii-graph-parser-padding,CHANGELOG.md,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Run broad gates, update docs/gap inventory, commit, and close or split follow-ons.
  Validation:
  - PASS `cargo nextest run -p merman-ascii`
  - PASS `cargo nextest run -p merman --features ascii`
  - PASS `cargo nextest run -p merman-cli --features ascii`
  - PASS `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - PASS `git diff --check`
  Evidence: `EVIDENCE_AND_GATES.md`, `JOURNAL/2026-05-29-agp-040.md`
  Handoff: Lane closed.
