# Flowchart ViewBox Node Bounds Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

The previous flowchart viewBox extraction moved rendered content bounds into `flowchart/viewbox.rs`,
but node and shape-specific bbox heuristics still dominated that module. Those heuristics are a
separate concern from cluster/edge/title viewBox orchestration.

## Target State

`flowchart/viewbox.rs` owns rendered-bounds orchestration. `flowchart/viewbox_node_bounds.rs` owns
node rendered-bounds preparation, including shape-specific RoughJS bbox approximations and layout
label metric fallback rules.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/viewbox_node_bounds.rs`.
- Move node/shape rendered-bounds preparation out of `viewbox.rs`.
- Consolidate repeated layout-node label measurement into one helper path.
- Keep RoughJS path bbox helpers local to node-bounds preparation.

## Out Of Scope

- Changing any shape parity heuristic.
- Changing edge curve geometry or final title bbox logic.
- Performance benchmarking.

## Closeout Condition

- `viewbox.rs` delegates node rendered-bounds preparation to the new module.
- Repeated label metrics calls are consolidated inside `viewbox_node_bounds.rs`.
- Flowchart DOM parity and package gates pass.
- Evidence records the current local machine and notes that historical benchmark data may have been
  collected on different hardware.
