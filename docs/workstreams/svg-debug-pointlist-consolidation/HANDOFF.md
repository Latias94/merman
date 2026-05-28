# SVG Debug Point-List Consolidation - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is complete. Low-risk debug SVG polyline emitters now use the shared point-list
helper.

## Completed Task

- Task ID: SDP-020
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity.rs`
  - `crates/merman-render/src/svg/parity/er.rs`
  - `crates/merman-render/src/svg/parity/flowchart/debug_svg.rs`
  - `crates/merman-render/src/svg/parity/class/debug_svg.rs`
  - `crates/merman-render/src/svg/parity/state/debug_svg.rs`
  - `crates/merman-render/src/svg/parity/sequence/debug.rs`
- Validation:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render debug_svg`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

- Task ID: SDP-030
- Owner: codex
- Files:
  - `docs/workstreams/svg-debug-pointlist-consolidation/*`
  - `docs/rendering/REFACTOR_TODO.md`
- Validation:
  - package gates in `EVIDENCE_AND_GATES.md`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions

- Keep this lane limited to debug polyline point-list emission.
- Do not touch block/state/flowchart large renderer module splits in this task.

## Next Recommended Action

- Continue helper consolidation with another bounded adopter, or move to the higher-value state
  renderer module split as a separate workstream.
