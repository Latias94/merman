# Flowchart Edge Geometry Compute Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`flowchart/mod.rs` should act as the flowchart renderer façade and entry-point hub. It still owned
the full edge path geometry implementation, including local Mermaid-style label-position helpers and
path normalization logic. That made the façade harder to scan and kept edge geometry split across
`mod.rs` and `edge_geom/*`.

## Target State

`flowchart/edge_geom/compute.rs` owns the edge path geometry computation. `flowchart/edge_geom.rs`
keeps the small public-in-flowchart façade, while `flowchart/mod.rs` returns to wiring entry points
and shared flowchart exports.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/edge_geom/compute.rs`.
- Move `flowchart_compute_edge_path_geom_impl` and its local Mermaid helper functions out of
  `flowchart/mod.rs`.
- Preserve the existing `flowchart_compute_edge_path_geom` call surface used by rendering and
  viewBox preparation.
- Keep visibility restricted to the flowchart renderer module tree.

## Out Of Scope

- Changing edge geometry behavior.
- Splitting the large `edge_geom/intersect.rs` file.
- Performance benchmarking.
- Touching the user's concurrent `fallback.rs` / resvg output pipeline work.

## Closeout Condition

- `flowchart/mod.rs` no longer owns edge path geometry computation.
- Flowchart DOM parity and package gates pass.
- Evidence records the current local machine and notes that historical benchmark data may have been
  collected on different hardware.
