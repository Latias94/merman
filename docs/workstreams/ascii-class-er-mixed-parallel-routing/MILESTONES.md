# ASCII Class ER Mixed Parallel Routing - Milestones

Status: Active
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to mixed-parallel relationship components.
- Cyclic, spanning-level, dense, and flowchart routing remain out of scope.

## M1 - Mixed Parallel Contract Tests

Exit criteria:

- Class and ER mixed-parallel behavior is specified by parser-backed tests.
- Tests fail before implementation on the old parallel diagnostics.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`

## M2 - Layered Parallel Lane Offsets

Exit criteria:

- Duplicate endpoint pairs are accepted by layered level assignment.
- Duplicate endpoint-pair lanes are visibly offset in class and ER drawing.
- Existing chain/star/crossing/component/same-endpoint parallel behavior remains stable.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Public support docs describe mixed-parallel support.
- Full package gate passes.
- Remaining topology work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`
