# ASCII Class ER Layered Planner - Milestones

Status: Closed
Last updated: 2026-05-30

## M0 - Lane Opening

Exit criteria:

- Scope is limited to shared planner extraction.
- Dense/crossing topology support remains out of scope.

Primary evidence:

- `DESIGN.md`
- `TODO.md`
- `WORKSTREAM.json`

## M1 - Class Adapter Extraction

Exit criteria:

- `relation_graph` exposes a generic layered planner over boxes and directed edges.
- classDiagram layered relationship rendering consumes that planner.
- Existing class chain, star, and crossing diagnostics stay stable.

Primary gates:

- `cargo nextest run -p merman-ascii class`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M2 - ER Adapter Extraction

Exit criteria:

- erDiagram layered relationship rendering consumes the same planner.
- ER cardinality, line style, and labels remain adapter-owned.
- Existing class behavior remains stable after ER joins the shared planner.

Primary gates:

- `cargo nextest run -p merman-ascii er`
- `cargo nextest run -p merman-ascii class`
- `cargo clippy -p merman-ascii --all-targets -- -D warnings`

## M3 - Closeout

Exit criteria:

- Full `merman-ascii` package gate passes.
- Evidence records behavior-preserving extraction.
- Any residual duplication is named or split.

Primary gates:

- `cargo nextest run -p merman-ascii`
- `cargo fmt --all --check`
- `git diff --check`

Primary evidence:

- Both class and ER layered renderers consume `relation_graph::plan_layered_relation_boxes`.
- Full `merman-ascii` package and lint gates pass.
- Dense/crossing topology support is explicitly deferred to a separate lane.
