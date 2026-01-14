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
- Implement `stateDiagram` (`stateDiagram-v2` renderer path) layout via `dugong` (Dagre-wrapper compatible).
- Implement `classDiagram` (`dagre-wrapper` renderer path) layout via `dugong` (Dagre-wrapper compatible).
- Support compound graphs for subgraphs by mapping Mermaid `subgraphs[]` to compound nodes.
- Use a pluggable `TextMeasurer` trait with a deterministic default measurer for CI.
- Emit explicit cluster layout information (box bounds + title placeholder) to make subgraph rendering
  backend-independent.
- The default flowchart behavior applies `flowchart.wrappingWidth` when measuring node/edge labels,
  matching Mermaid's `createText` width parameter usage. When `flowchart.htmlLabels=true`, use an
  HTML-like wrapping mode (no long-word splitting, width clamped to max-width, line-height 1.5).
- Subgraph title placeholders use Mermaid's `createText` default width (200) for wrapping.
- For isolated, leaf-only clusters (no external edges), apply a Mermaid-like "cluster dir" behavior:
  the cluster's `dir` (or toggled direction when `inheritDir=false`) influences the internal layout
  of its member nodes.
  - Root, isolated clusters are packed after the recursive step to avoid overlaps, approximating
    Mermaid's `clusterNode` behavior (recursive render updates cluster bounds before the parent
    graph is laid out).
- For `flowchart-v2` edge labels, mimic Mermaid's modern Dagre pipeline by inserting a label node
  and splitting the labeled edge into two edges internally; the public layout output still reports
  the original edge id, route and label position.
  - Label nodes are assigned to the lowest common compound parent of their endpoints (when any),
    and cluster bounds include those label nodes.
- For `classDiagram` relations, include Mermaid-style edge terminal label positions (e.g. cardinalities)
  using `start_label_*` / `end_label_*` slots, positioned via Mermaid's `calcTerminalLabelPosition` logic.

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
- Clusters expose `requested_dir` (from the semantic model) and `effective_dir` (the rankdir actually
  used for isolated cluster layout).
- Clusters expose Mermaid parity fields used by the SVG renderers:
  - `diff`: the Mermaid cluster "diff" value (see `packages/mermaid/src/rendering-util/rendering-elements/clusters.js`)
  - `offset_y`: the Mermaid cluster "offsetY" value (`labelBBox.height - padding/2`)

See also: `docs/rendering/FLOWCHART_DEBUG_SVG.md`.
See also: `docs/rendering/STATE_DEBUG_SVG.md`.
See also: `docs/rendering/CLASS_DEBUG_SVG.md`.
