# ASCII Graph Junction Routing - TODO

Status: Complete
Last updated: 2026-05-29

## M0 - Scope And Evidence Freeze

- [x] AGJ-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-graph-junction-routing]
  Goal: Open the follow-up lane and freeze the module/routing boundary.
  Validation: DESIGN.md, TODO.md, MILESTONES.md, EVIDENCE_AND_GATES.md, WORKSTREAM.json, and HANDOFF.md exist and agree.
  Evidence: `docs/workstreams/ascii-graph-junction-routing/DESIGN.md`
  Handoff: AGJ-020 is next.

## M1 - Graph Module Split

- [x] AGJ-020 [owner=codex] [deps=AGJ-010] [scope=crates/merman-ascii/src/graph]
  Goal: Split charset, layout, draw, and routing responsibilities out of `graph/mod.rs` without intentional behavior changes.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
  Review: `adapter.rs` remains the only graph module that depends on `FlowchartV2Model`.
  Evidence: `graph/mod.rs` now only wires modules and tests; renderer responsibilities moved to
  `charset.rs`, `layout.rs`, `draw.rs`, and `routing.rs`.
  Handoff: AGJ-030 is next if behavior-preserving gates pass.

## M2 - Junction Merge Routing

- [x] AGJ-030 [owner=codex] [deps=AGJ-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Add junction-aware line merging for existing LR edge routes and move exact crossing fixtures into the allowlist.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
  Review: Do not special-case fixture names; route merging must be glyph/segment based.
  Evidence: LR routing now uses a Go-style grid path search for non-self forward/crossing edges
  and route-cell junction merging. Exact graph fixture matches increased from 44 to 48.
  Handoff: AGJ-040 is next.

## M3 - Closeout

- [x] AGJ-040 [owner=codex] [deps=AGJ-030] [scope=docs/workstreams/ascii-graph-junction-routing,CHANGELOG.md,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Record final evidence, document shipped behavior and remaining gaps, verify, and commit.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii` (43 passed)
  - PASS 2026-05-29: `cargo nextest run -p merman --features ascii` (3 passed)
  - PASS 2026-05-29: `cargo nextest run -p merman-cli --features ascii` (10 passed)
  - PASS 2026-05-29: `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - PASS 2026-05-29: `git diff --check`
  Review: Remaining non-junction work is split or named as follow-up.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: Lane closed. Follow-ons are label lanes, TD back-edge labels, padding fixture directives,
  and complex subgraph routing.
