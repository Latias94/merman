# ASCII Class ER Component Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-topology-routing/` lane. Class and ER
can render layered chains, stars, adjacent-layer crossings, and one related component plus unrelated
standalone classes/entities.

## Active Task

- Task ID: ACECL-040
- Owner: unassigned
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-component-layout/`
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Support docs should describe disconnected component support without claiming dense,
  cyclic, parallel, or spanning-level routing.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Only disconnected component layout is in scope.
- Keep parallel, cyclic, spanning-level, and dense routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Relationship-bearing boxes intentionally stay in one layered planner domain; isolated boxes split
  into standalone output components.
