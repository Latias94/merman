# Headless Layout (Stage A)

This document describes the first rendering milestone for `merman`:
produce a deterministic **headless layout model** (node positions + edge routes) without
generating SVG.

Baseline: Mermaid `@11.12.2`.

## Goals

- Pure Rust, no DOM.
- Output is a stable, serializable layout model that other UI frameworks can consume.
- Keep the path open for later SVG generation (Stage B) without redesigning the core layout API.

## Current Scope

- Implement `flowchart-v2` layout via `dugong` (Dagre-compatible).
- Support compound graphs for subgraphs by mapping Mermaid `subgraphs[]` to compound nodes.
- Use a pluggable `TextMeasurer` trait with a deterministic default measurer for CI.
- Emit explicit cluster layout information (box bounds + title placeholder) to make subgraph rendering
  backend-independent.

## API

Primary entrypoint:

- `merman_render::layout_parsed(&ParsedDiagram, &LayoutOptions) -> LayoutedDiagram`

The result includes:

- `meta`: diagram type + config/effective config
- `semantic`: the original semantic JSON model (from `merman-core`)
- `layout`: a diagram-specific layout structure (`FlowchartV2Layout` for now)

## Notes

- The deterministic `TextMeasurer` is a placeholder for parity-driven measurement.
  Full SVG parity will require faithful measurement and per-shape sizing rules.
- Cluster title placement uses Mermaid-compatible `flowchart.subGraphTitleMargin.{top,bottom}`.
