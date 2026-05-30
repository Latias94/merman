# ASCII Class ER Spanning Level Routing - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to non-cyclic spanning-level relationships.
- Dense, cyclic, and flowchart routing remain out of scope.

## M1 - Spanning Contract Tests

Exit criteria:

- Class and ER spanning-level behavior is specified by parser-backed tests.
- Tests fail before implementation on the old spanning-level diagnostics.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`

## M2 - Side-Lane Spanning Routing

Exit criteria:

- Non-cyclic spanning-level edges are accepted by layered planning.
- Spanning edges route around intermediate boxes through side lanes.
- Existing chain/star/crossing/component/parallel behavior remains stable.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Public support docs describe spanning-level support.
- Full package gate passes.
- Remaining topology work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`

Closeout result:

- Public support docs now describe simple spanning-level class/ER side lanes.
- Full `merman-ascii` package tests, lint, fmt, and whitespace gates passed on 2026-05-30.
- Remaining topology work stays outside this lane: cyclic layouts and dense label/marker collision
  routing.
