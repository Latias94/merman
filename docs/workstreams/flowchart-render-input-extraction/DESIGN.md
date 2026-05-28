# Flowchart Render Input Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`flowchart/svg_emit.rs` still prepares render-time edges and helper nodes inline. That preparation
includes Mermaid self-loop expansion and cluster-edge DOM ordering, both of which are model-to-SVG
input adaptation rather than SVG document emission.

## Target State

`flowchart/render_input.rs` owns render-time edge/helper-node preparation. `svg_emit.rs` receives a
prepared value and continues with context building and SVG emission.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/render_input.rs`.
- Move self-loop helper edge expansion, self-loop helper node creation, and cluster edge DOM order
  partitioning out of `svg_emit.rs`.
- Preserve render edge order and SVG output.

## Out Of Scope

- Changing self-loop helper edge generation in `crate::flowchart`.
- Moving viewBox bounds logic.
- Moving edge path rendering.
- Performance benchmarking.

## Closeout Condition

- `svg_emit.rs` no longer owns render input expansion.
- Flowchart parity and package gates pass.
- Evidence records the current local machine.
