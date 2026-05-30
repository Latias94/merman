# ASCII Class ER Parallel Routing - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane follows the closed class/ER layered planner, topology routing, and component layout lanes.
Class and ER can render chains, stars, adjacent-layer crossings, isolated standalone components, and
simple same-endpoint parallel relationships.

## Final State

- Completed tasks: ACEPR-010, ACEPR-020, ACEPR-030, ACEPR-040.
- Support docs: `README.md`, `crates/merman-cli/README.md`,
  `crates/merman-ascii/README.md`.
- Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`; `git diff --check`.
- Evidence: `EVIDENCE_AND_GATES.md`.

## Residual Follow-Ons

- Mixed endpoint pairs with parallel edges remain dense-routing work.
- Cyclic layouts and spanning-level edges remain separate topology work.
- Class/ER relationship semantics remain outside `relation_graph`.
- Flowchart routing remains out of scope.
