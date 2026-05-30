# ASCII Class ER Spanning Level Routing - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane follows the closed mixed-parallel lane. Class and ER can render useful layered,
crossing-by-reorder, component, parallel, and simple spanning-level layouts. A non-cyclic edge that
skips one intermediate level now routes through a reserved side lane instead of crossing the
intermediate box.

## Active Task

- Task ID: ACESLR-040
- Owner: unassigned
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-spanning-level-routing`
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`
- Status: READY
- Review: Public docs should describe simple spanning-level side-lane support without claiming
  cyclic, dense, color/style, or flowchart routing support.
- Evidence: `EVIDENCE_AND_GATES.md`

## Constraints

- Only non-cyclic spanning-level class/ER relationships are in scope.
- Keep dense, cyclic, and flowchart routing separate.
- Keep class/ER relationship semantics out of `relation_graph`.
- Do not route a spanning edge through an intermediate box.
