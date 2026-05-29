# ASCII Graph Final Parity - Handoff

Status: Complete
Last updated: 2026-05-29

## Current State

AGF-050 is complete. Current exact graph fixture count is 75: 52 ASCII and 23 Unicode. Graph fixture
gaps are clear for all copied `mermaid-ascii` graph fixtures.

## Active Task

- Task ID: none
- Owner: codex
- Files: none
- Validation: Broad gates passed.
- Status: COMPLETE
- Review: No active task remains in this workstream.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Do routing module deepening before multiline or subgraph behavior.
- Split route-cell merging into `graph/routing/cell.rs` and routed-label placement into
  `graph/routing/label.rs`; keep `routing.rs` as route orchestration.
- Added `GraphLabel` for graph node labels so layout and drawing share line-aware width/height
  semantics.
- Added nested subgraph bounds and Mermaid-ascii-compatible subgraph layout/routing rules; all
  copied graph fixtures are exact.
- Keep remaining subgraph parity in this workstream but split blockers if exact parity becomes
  larger than one lane.

## Blockers

- None.

## Next Recommended Action

- No next action in this workstream. Future work should target new copied fixtures or non-fixture
  product gaps as a separate lane.
