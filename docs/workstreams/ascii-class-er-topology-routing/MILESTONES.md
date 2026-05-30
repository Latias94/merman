# ASCII Class ER Topology Routing - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to class/ER topology routing.
- Crossing is the first slice; denser graph support remains deferred.

Primary evidence:

- `DESIGN.md`
- `TODO.md`
- `WORKSTREAM.json`

## M1 - Crossing Contract Tests

Exit criteria:

- Class and ER crossing behavior is specified by parser-backed public tests.
- Tests fail before implementation because crossing is still unsupported.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`

## M2 - Shared Crossing Planner

Exit criteria:

- Adjacent-layer crossing edges render without dropping relationships.
- Unrelated, cyclic, parallel, and spanning-level topology diagnostics remain explicit.
- Class and ER adapters still own typed semantics.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo nextest run -p merman-ascii er`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Docs And Closeout

Exit criteria:

- Support docs describe crossing support and remaining diagnostics.
- Full package gate passes.
- Remaining dense topology work is split or deferred.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`

Primary evidence:

- Public docs describe adjacent-layer crossing support by layer reordering.
- Full `merman-ascii` package and lint gates pass.
- Remaining dense topology work is deferred.
