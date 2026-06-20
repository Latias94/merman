---
type: Work Progress
status: active
---

# Log

## 2026-06-18
- Verified source-backed Flowchart ELK probes are green.
- Ported compound parent-end external dummy net-flow handling in `merman-elk-layered` closer to ELK `calculateNetFlow` behavior.
- Added regression coverage for parent-end external dummy net-flow behavior and existing compound metadata tests still pass.
- Ported inside-self-loop handling so ELK `insideSelfLoops.activate` nodes create nested graphs and `inside_self_loops_yo` edges are imported into the source node nested graph.
- Added regression coverage for inside-self-loop nested graph creation and kept source-backed probe coverage green.
- Verified `cargo test -p merman-elk-layered --tests`, `cargo test -p merman-layout-elk --tests`, `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`, and `cargo fmt --all`.
