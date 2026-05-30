# ASCII Class ER Component Layout - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane follows the closed `docs/workstreams/ascii-class-er-topology-routing/` lane. Class and ER
can render layered chains, stars, adjacent-layer crossings, and one related component plus unrelated
standalone classes/entities.

## Final State

- Completed tasks: ACECL-010, ACECL-020, ACECL-030, ACECL-040.
- Support docs: `README.md`, `crates/merman-cli/README.md`,
  `crates/merman-ascii/README.md`.
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`.
- Evidence: `EVIDENCE_AND_GATES.md`.

## Residual Follow-Ons

- Parallel, cyclic, spanning-level, and dense class/ER relationship routing remain separate topology
  work.
- Relationship-bearing boxes intentionally stay in one layered planner domain; isolated boxes split
  into standalone output components. A future full graph-layout lane can revisit this policy if it
  adds true multi-component horizontal packing.
- Class/ER relationship semantics remain outside `relation_graph`.
