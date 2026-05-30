# ASCII Class ER Spanning Level Routing - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane follows the closed mixed-parallel lane. Class and ER can render useful layered,
crossing-by-reorder, component, parallel, and simple spanning-level layouts. A non-cyclic edge that
skips one intermediate level now routes through a reserved side lane instead of crossing the
intermediate box.

## Final State

- Completed tasks: ACESLR-010, ACESLR-020, ACESLR-030, ACESLR-040.
- Support docs: `README.md`, `crates/merman-cli/README.md`,
  `crates/merman-ascii/README.md`.
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`.
- Evidence: `EVIDENCE_AND_GATES.md`.

## Residual Follow-Ons

- Cyclic layouts remain separate topology work.
- Dense label/marker collision routing remains a separate lane.
- More complex topology routing beyond this simple non-cyclic spanning-level subset remains follow-on
  scope.
- Keep class/ER relationship semantics out of `relation_graph`.
- Flowchart routing remains out of scope.
