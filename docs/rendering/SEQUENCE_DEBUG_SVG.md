# Sequence Diagram Debug SVG

This debug exporter is for development only. It visualizes the **headless layout output**
from `merman-render` for Mermaid `sequence` diagrams (Mermaid `@11.12.2`).

It is **not** intended to be Mermaid-parity SVG output (Stage B). It exists to quickly spot layout
issues (overlaps, broken routes, incorrect label placement).

## Run

Single fixture:

`cargo run -p xtask -- gen-debug-svgs --diagram sequence --filter basic`

Outputs to: `target/debug-svgs/sequence/basic.svg`

## Bulk export (all fixtures)

`cargo run -p xtask -- gen-debug-svgs --diagram sequence`

Outputs to: `target/debug-svgs/sequence/*.svg`

## What It Shows

- Blue rectangles: layout nodes (actors + helper nodes).
- Black polylines: layout edges (messages + lifelines).
- Clusters are currently unused for sequence.
