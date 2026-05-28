# ASCII Graph Routing Parity - TODO

Status: Complete
Last updated: 2026-05-29

## M0 - Workstream And Baseline

- [x] AGR-010 [owner=codex] [deps=none] [scope=docs/workstreams/ascii-graph-routing-parity,crates/merman-ascii/src/graph]
  Goal: Open the routing parity lane and split `graph/mod.rs` into stable internal module
  boundaries without changing behavior.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 19 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph::` (6 passed, 35 skipped)
  Review: Module split must keep `merman-core` dependencies inside the adapter boundary.
  Evidence: `graph/mod.rs` now delegates model types to `graph/model.rs` and
  `FlowchartV2Model` conversion to `graph/adapter.rs`.
  Handoff: AGR-020 is next.

## M1 - Fixture Harness

- [x] AGR-020 [owner=codex] [deps=AGR-010] [scope=crates/merman-ascii/tests,crates/merman-ascii/tests/testdata/mermaid-ascii]
  Goal: Add a graph fixture harness with an explicit allowlist and skipped-gap inventory for copied
  `mermaid-ascii` graph fixtures.
  Validation:
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  Review: The harness must not claim full corpus parity; unsupported fixtures must be named.
  Evidence: `tests/graph_fixture.rs` allowlists 13 exact graph fixture matches and requires all
  remaining copied graph fixtures to be named as gaps. `GRAPH_FIXTURE_GAPS.md` mirrors the readable
  gap inventory.
  Handoff: AGR-030 is next.

## M2 - Branch And Multi-Root Routing

- [x] AGR-030 [owner=codex] [deps=AGR-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Replace the linear-only layout assumption for common multi-root, branch, fan-out, and
  fan-in flowcharts.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph::` (6 passed, 37 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii` (43 passed)
  - PASS 2026-05-29: `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Current simple LR/TD outputs should remain stable unless a fixture-backed change is
  intentional.
  Evidence: LR graph layout now uses a reference-style 3x3 grid for roots and child levels.
  Allowlisted exact graph fixtures increased from 13 to 31.
  Handoff: AGR-040 is next.

## M3 - Back Edges And Self References

- [x] AGR-040 [owner=codex] [deps=AGR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Add routing support for self references, back edges, non-adjacent edges, and label
  separation where feasible inside the time box.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii` (43 passed)
  - PASS 2026-05-29: `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Avoid silent misrouting; split fixtures that need deeper junction merging.
  Evidence: Self-loop and same-row back-edge routing moved six more exact fixtures into the
  allowlist, raising graph fixture parity from 31 to 37 exact matches.
  Handoff: AGR-050 is next.

## M4 - Subgraph Routing Hardening

- [x] AGR-050 [owner=codex] [deps=AGR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Harden multiple/nested subgraph layout where feasible, and document remaining complex
  cases.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii graph_fixture` (2 passed, 41 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii flowchart` (22 passed, 21 skipped)
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii` (43 passed)
  - PASS 2026-05-29: `cargo clippy -p merman-ascii --all-targets -- -D warnings`
  Review: Complex nested/external-edge cases may be split if they threaten routing stability.
  Evidence: Upstream-style simple subgraph title rows and empty-subgraph offset handling moved
  seven more ASCII fixtures into the allowlist, raising graph fixture parity from 37 to 44 exact
  matches.
  Handoff: AGR-060 is next.

## M5 - Closeout

- [x] AGR-060 [owner=codex] [deps=AGR-020] [scope=docs/workstreams/ascii-graph-routing-parity,crates/merman-ascii/FLOWCHART_SUPPORT.md,CHANGELOG.md]
  Goal: Record final evidence, commit completed bounded tasks, and close or hand off the lane with
  concrete remaining fixture gaps.
  Validation:
  - PASS 2026-05-29: `cargo fmt --all --check`
  - PASS 2026-05-29: `cargo nextest run -p merman-ascii` (43 passed)
  - PASS 2026-05-29: `cargo nextest run -p merman --features ascii` (3 passed)
  - PASS 2026-05-29: `cargo nextest run -p merman-cli --features ascii` (10 passed)
  - PASS 2026-05-29: `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - PASS 2026-05-29: `git diff --check`
  Review: Close only if shipped slices are verified and remaining gaps are named.
  Evidence: `docs/workstreams/ascii-graph-routing-parity/EVIDENCE_AND_GATES.md`; final allowlist
  is 44 exact graph fixtures, with remaining gaps named in
  `crates/merman-ascii/tests/testdata/mermaid-ascii/GRAPH_FIXTURE_GAPS.md`.
  Handoff: Lane closed with explicit follow-ons.

## 09:00 Execution Order

1. AGR-010: module split first, because routing work needs boundaries.
2. AGR-020: fixture harness second, because future routing needs measurable gaps.
3. AGR-030: branch/multi-root support if enough runway remains.
4. AGR-040 or AGR-050: choose based on fixture failures after AGR-030.
5. AGR-060: closeout or handoff before 09:00.
