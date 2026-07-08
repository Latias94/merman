---
type: Engineering Log
---

# Log

## 2026-07-04

- Consolidated editor-language hardening around SVG DOM safety, VS Code preview refresh
  reliability, reusable binding lifecycle docs, editor snapshot text sharing, workflow path gates,
  and web script argument validation.
- Public platform docs now treat document analysis/facts and reusable-engine callback lifecycle as
  part of the wrapper contract instead of implementation trivia.

## 2026-06-18

- Verified source-backed Flowchart ELK probes are green.
- Ported compound parent-end external dummy net-flow handling in `merman-elk-layered` closer to ELK
  `calculateNetFlow` behavior.
- Added regression coverage for parent-end external dummy net-flow behavior and existing compound
  metadata tests still pass.
- Ported inside-self-loop handling so ELK `insideSelfLoops.activate` nodes create nested graphs and
  `inside_self_loops_yo` edges are imported into the source node nested graph.
- Added regression coverage for inside-self-loop nested graph creation and kept source-backed probe
  coverage green.
- Verified `cargo test -p merman-elk-layered --tests`, `cargo test -p merman-layout-elk --tests`,
  `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`, and `cargo fmt --all`.
