# Class Diagram Debug SVG

This debug exporter is for development only. It visualizes the **headless layout output**
from `merman-render` for Mermaid `classDiagram` (Mermaid `@11.12.2`).

It is **not** intended to be Mermaid-parity SVG output (Stage B). It exists to make it easy to
spot obvious layout issues (overlaps, broken routes, missing cluster bounds, terminal label placement).

## Run

PowerShell:

`Get-Content fixtures\\class\\basic.mmd | cargo run -p merman-render --example class_debug_svg > out.svg`

With cardinalities (terminal labels):

`Get-Content fixtures\\class\\upstream_relation_types_and_cardinalities_spec.mmd | cargo run -p merman-render --example class_debug_svg > out.svg`

## What It Shows

- Blue rectangles: node bounding boxes (classes + notes).
- Gray rectangles: namespace clusters.
- Black polylines: edge routes.
- Yellow boxes: edge label bounding boxes (`edge.title`).
- Cyan boxes: edge terminal label bounding boxes (e.g. cardinalities), labeled as `SL/SR/EL/ER`.

