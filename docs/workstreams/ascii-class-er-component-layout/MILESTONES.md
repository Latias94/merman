# ASCII Class ER Component Layout - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to disconnected components.
- Denser topology routing remains out of scope.

## M1 - Component Contract Tests

Exit criteria:

- Class and ER disconnected component behavior is specified by parser-backed tests.
- Tests fail before implementation on the old unrelated diagnostics.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`

## M2 - Shared Component Partition

Exit criteria:

- Shared component partitioning is used by class and ER.
- Each component reuses no-edge, single-edge, or layered rendering paths.
- Existing chain/star/crossing behavior remains stable.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Public support docs describe disconnected component support.
- Full package gate passes.
- Remaining topology work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`

Closeout result:

- Public support docs now describe unrelated standalone class/entity components.
- Full `merman-ascii` package tests, lint, fmt, and whitespace gates passed on 2026-05-30.
- Remaining topology work stays outside this lane: parallel, cyclic, spanning-level, and dense
  relationship routing.
