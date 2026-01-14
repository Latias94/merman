# Flowchart Debug SVG

Baseline: Mermaid `@11.12.2`.

This is a **developer tool** that turns the headless `flowchart-v2` layout output into a simple SVG
for visual inspection. It is **not** the parity renderer (Stage B).

## Usage

From the workspace root:

```bash
cargo run -p merman-render --example flowchart_debug_svg < fixtures/flowchart/basic.mmd > out.svg
```

## What it Draws

- Cluster bounding boxes and titles
- Node bounding boxes and ids
- Edge polylines (routes) and edge id labels

## Mermaid Metadata Visualization

Each cluster `<g>` includes:

- `data-diff`: Mermaid cluster `diff` (see `packages/mermaid/src/rendering-util/rendering-elements/clusters.js`)
- `data-offset-y`: Mermaid cluster `offsetY` (`labelBBox.height - padding/2`)

The SVG can optionally draw a small red cross to visualize the **clusterNode translation origin**
used by Mermaid's `positionNode(...)` (`packages/mermaid/src/rendering-util/rendering-elements/nodes.ts`).
This is disabled by default in `SvgRenderOptions`.
