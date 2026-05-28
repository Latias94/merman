# Flowchart Geometry Helper Consolidation

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`flowchart/svg_emit.rs` carries duplicate point-generation helpers for rendered-bounds estimation.
The same Mermaid geometry helpers already live under `flowchart/render/node/geom.rs` for actual
node rendering.

## Target State

The renderer and viewBox bounds path share one implementation for the common Mermaid point
generators. `svg_emit.rs` keeps only bounds orchestration and local rough-path bounds helpers.

## In Scope

- Widen `flowchart/render/node/geom.rs` visibility only inside the flowchart renderer.
- Remove duplicated `generate_circle_points` and `generate_full_sine_wave_points` from
  `svg_emit.rs`.
- Preserve rendered SVG output.

## Out Of Scope

- Reworking the large viewBox bounds loop.
- Touching edge intersection helpers.
- Performance benchmarking.

## Closeout Condition

- Duplicate helpers are removed from `svg_emit.rs`.
- Flowchart parity and package gates pass.
- `REFACTOR_TODO.md` records the completed cleanup.
