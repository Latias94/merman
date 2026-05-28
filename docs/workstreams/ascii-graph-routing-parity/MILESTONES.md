# ASCII Graph Routing Parity - Milestones

Status: Active
Last updated: 2026-05-29

## M0 - Workstream And Baseline

Exit criteria:

- Workstream docs exist.
- `graph/mod.rs` is split into durable module boundaries.
- Focused behavior gates remain green.

## M1 - Fixture Harness

Exit criteria:

- Reference graph fixtures have an allowlist-based Rust harness.
- Unsupported copied fixtures are inventoried as gaps instead of ignored implicitly.

## M2 - Branch And Multi-Root Routing

Status: Complete for the current lane slice.

Exit criteria:

- At least one branch or multi-root reference fixture moves into the passing allowlist.
- Existing basic graph output remains stable or intentionally updated.

## M3 - Back Edges And Self References

Exit criteria:

- At least one self-reference or back-edge fixture moves into the passing allowlist, or the lane
  records why this must be split.

## M4 - Subgraph Routing Hardening

Exit criteria:

- At least one multiple/nested subgraph fixture is supported, or the lane records a follow-on with
  concrete blockers.

## M5 - Closeout

Exit criteria:

- Fresh gates are recorded.
- Completed tasks are committed.
- Remaining gaps are listed by fixture family.
