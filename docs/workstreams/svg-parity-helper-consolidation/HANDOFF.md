# SVG Parity Helper Consolidation - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

Workstream complete for the first bounded task. Radar SVG polygon point-list emission now uses the
shared point formatting helper.

## Completed Tasks

- Task ID: SPH-020
- Owner: codex
- Files:
  - `crates/merman-render/src/svg/parity/util.rs`
  - `crates/merman-render/src/svg/parity/radar.rs`
- Validation:
  - `cargo nextest run -p merman-render fmt_points`
  - `cargo nextest run -p merman-render radar`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

- Task ID: SPH-030
- Owner: codex
- Files:
  - `docs/workstreams/svg-parity-helper-consolidation/*`
- Validation:
  - package gates in `EVIDENCE_AND_GATES.md`
- Status: DONE
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions

- Do not touch state/flowchart large files in the first task.
- Keep generated overrides unchanged.
- Preserve exact point separator and formatting semantics.

## Next Recommended Action

- Split the next helper adopter as a separate bounded task. Good candidates are ER/debug SVG
  point-list emitters, then block once the helper has more coverage.
