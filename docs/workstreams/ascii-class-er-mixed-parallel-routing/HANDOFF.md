# ASCII Class ER Mixed Parallel Routing - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane follows the closed same-endpoint parallel lane. Class and ER can render a component where
a duplicate endpoint pair is mixed with another ordinary edge.

## Final State

- Completed tasks: ACEMPR-010, ACEMPR-020, ACEMPR-030, ACEMPR-040.
- Support docs: `README.md`, `crates/merman-cli/README.md`,
  `crates/merman-ascii/README.md`.
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`.
- Evidence: `EVIDENCE_AND_GATES.md`.

## Residual Follow-Ons

- Cyclic layouts and spanning-level edges remain separate topology work.
- Dense label/marker collision routing remains a separate lane.
- Class/ER relationship semantics remain outside `relation_graph`.
- Flowchart routing remains out of scope.
