# ER Diagram Debug SVG

This debug exporter is for development only. It visualizes the **headless layout output**
from `merman-render` for Mermaid `erDiagram` (Mermaid `@11.12.2`).

It is **not** intended to be Mermaid-parity SVG output (Stage B). It exists to quickly spot layout
issues (overlaps, broken routes, incorrect label placement).

## Run

PowerShell:

`Get-Content fixtures\\er\\basic.mmd | cargo run -p merman-render --example er_debug_svg > out.svg`

## Bulk export (all fixtures)

`cargo run -p xtask -- gen-debug-svgs --diagram er`

Outputs to: `target/debug-svgs/er/*.svg`

## What It Shows

- Blue rectangles: entity bounding boxes.
- Black polylines: relationship routes.
- Yellow boxes: relationship label bounding boxes (role text).

