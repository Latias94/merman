# ASCII Class ER Parallel Routing - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to same-endpoint parallel relationships.
- Cyclic, spanning-level, dense, and flowchart routing remain out of scope.

## M1 - Parallel Contract Tests

Exit criteria:

- Class and ER parallel behavior is specified by parser-backed tests.
- Tests fail before implementation on the old parallel diagnostics.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`

## M2 - Shared Parallel Lane Helper

Exit criteria:

- Simple same-endpoint parallel relationships render with distinct lanes.
- Each class relationship preserves marker, line style, and label.
- Each ER relationship preserves cardinality, line style, and label.
- Existing chain/star/crossing/component behavior remains stable.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Public support docs describe the same-endpoint parallel subset.
- Full package gate passes.
- Remaining topology work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`

Closeout result:

- Public support docs now describe same-endpoint class/ER parallel relationship lanes.
- Full `merman-ascii` package tests, lint, fmt, and whitespace gates passed on 2026-05-30.
- Remaining topology work stays outside this lane: mixed-parallel endpoint pairs, cyclic layouts,
  spanning-level routing, and dense relationship routing.
