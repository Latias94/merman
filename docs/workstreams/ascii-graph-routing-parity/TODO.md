# ASCII Graph Routing Parity - TODO

Status: Active
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

- [ ] AGR-020 [owner=codex] [deps=AGR-010] [scope=crates/merman-ascii/tests,crates/merman-ascii/tests/testdata/mermaid-ascii]
  Goal: Add a graph fixture harness with an explicit allowlist and skipped-gap inventory for copied
  `mermaid-ascii` graph fixtures.
  Validation:
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Review: The harness must not claim full corpus parity; unsupported fixtures must be named.
  Evidence: Allowlisted fixture tests and gap list.
  Handoff: Pending.

## M2 - Branch And Multi-Root Routing

- [ ] AGR-030 [owner=codex] [deps=AGR-020] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Replace the linear-only layout assumption for common multi-root, branch, fan-out, and
  fan-in flowcharts.
  Validation:
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Review: Current simple LR/TD outputs should remain stable unless a fixture-backed change is
  intentional.
  Evidence: Newly allowlisted reference graph fixtures.
  Handoff: Pending.

## M3 - Back Edges And Self References

- [ ] AGR-040 [owner=codex] [deps=AGR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests]
  Goal: Add routing support for self references, back edges, non-adjacent edges, and label
  separation where feasible inside the time box.
  Validation:
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Review: Avoid silent misrouting; split fixtures that need deeper junction merging.
  Evidence: Reference fixture allowlist expansion.
  Handoff: Pending.

## M4 - Subgraph Routing Hardening

- [ ] AGR-050 [owner=codex] [deps=AGR-030] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests,crates/merman-ascii/FLOWCHART_SUPPORT.md]
  Goal: Harden multiple/nested subgraph layout where feasible, and document remaining complex
  cases.
  Validation:
  - `cargo nextest run -p merman-ascii graph_fixture`
  - `cargo nextest run -p merman-ascii flowchart`
  Review: Complex nested/external-edge cases may be split if they threaten routing stability.
  Evidence: Subgraph fixture allowlist and support matrix updates.
  Handoff: Pending.

## M5 - Closeout

- [ ] AGR-060 [owner=codex] [deps=AGR-020] [scope=docs/workstreams/ascii-graph-routing-parity,crates/merman-ascii/FLOWCHART_SUPPORT.md,CHANGELOG.md]
  Goal: Record final evidence, commit completed bounded tasks, and close or hand off the lane with
  concrete remaining fixture gaps.
  Validation:
  - `cargo fmt --all --check`
  - `cargo nextest run -p merman-ascii`
  - `cargo nextest run -p merman --features ascii`
  - `cargo nextest run -p merman-cli --features ascii`
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
  - `git diff --check`
  Review: Close only if shipped slices are verified and remaining gaps are named.
  Evidence: `docs/workstreams/ascii-graph-routing-parity/EVIDENCE_AND_GATES.md`.
  Handoff: Pending.

## 09:00 Execution Order

1. AGR-010: module split first, because routing work needs boundaries.
2. AGR-020: fixture harness second, because future routing needs measurable gaps.
3. AGR-030: branch/multi-root support if enough runway remains.
4. AGR-040 or AGR-050: choose based on fixture failures after AGR-030.
5. AGR-060: closeout or handoff before 09:00.
