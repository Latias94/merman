# ASCII Class ER Mixed Parallel Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed same-endpoint parallel lane. Class and ER can render a component where
a duplicate endpoint pair is mixed with another ordinary edge.

## Active Task

- Task ID: ACEMPR-040
- Owner: unassigned
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-mixed-parallel-routing/`
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Support docs should describe mixed-parallel support without claiming cyclic,
  spanning-level, or dense collision routing.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Only mixed-parallel class/ER relationship components are in scope.
- Keep cyclic, spanning-level, dense, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not silently merge, overwrite, or omit parallel relationships.
