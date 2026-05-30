# ASCII Class ER Parallel Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed class/ER layered planner, topology routing, and component layout lanes.
Class and ER can render chains, stars, adjacent-layer crossings, isolated standalone components, and
simple same-endpoint parallel relationships.

## Active Task

- Task ID: ACEPR-040
- Owner: unassigned
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-parallel-routing/`
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Support docs should describe the same-endpoint parallel subset without claiming general
  dense, cyclic, or spanning-level routing.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Only same-endpoint parallel class/ER relationships are in scope.
- Keep cyclic, spanning-level, dense, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not silently merge, overwrite, or omit parallel relationships.
