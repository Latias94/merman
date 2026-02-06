# State Root ViewBox Parity Gaps

This document tracks known causes and debugging workflows for `stateDiagram-v2` root viewport
(`viewBox` + `style="max-width: ...px"`) mismatches against the upstream Mermaid `@11.12.2` SVG
baselines.

In Mermaid `@11.12.2`, the final root viewport is derived from DOM `svg.getBBox()` plus a fixed
padding (typically `8px`). Any geometry that survives into the final SVG tree can affect the root
viewport, including placeholder elements.

## Common Causes

### 1) Dagre coordinate drift (edge `data-points` differ)

If the upstream and local `data-points` arrays differ for the same edge/path id, the mismatch is a
layout drift (dugong vs. upstream dagre-d3-es), not a bbox parser issue.

This often manifests as:

- root `max-width` mismatch of ~0.1–10px
- root `viewBox` mismatch (usually width/height)
- `debug-svg-bbox` showing min/max contributors as edge `<path>` elements (e.g. `id="edge1"`)

### 1b) Missing endpoint intersection (shape-specific `data-points` drift)

Mermaid adjusts the first/last edge points by intersecting the polyline with the node's rendered
shape (e.g. start/end circles, rounded rects, **choice diamonds**). If the Rust renderer leaves
those endpoints at the node center, `data-points` will drift even when the underlying Dagre layout
matches.

This is usually fixed in the state layout post-process (`crates/merman-render/src/state.rs`) by
applying Mermaid-compatible intersection routines to the first/last points.

### 2) Rounded `rect` padding mismatch (rx/ry -> `roundedRect`)

Mermaid's state diagram node sizing treats `rect` nodes with `rx/ry` (converted to a `roundedRect`)
as having the full `state.padding` applied on the x-axis, i.e. **`pad_x = padding` per side**.

If the Rust layout underestimates this padding (for example by using `padding/2 - 1` per side), it
typically manifests as:

- root `max-width` mismatch of exactly `10px` (missing `5px` per side)
- node/edge x-coordinates shifted by `~5px` (Dagre keeps the left graph margin fixed)

This is *not* a bbox parser bug; it's a node sizing mismatch feeding Dagre.

### 3) Self-loop helper nodes / placeholders

Mermaid's dagre wrapper expands self-loop transitions by injecting helper nodes
`${nodeId}---${nodeId}---{1|2}` and extra `0.1 x 0.1` placeholder rects whose placement is derived
from those helper nodes. These placeholders can influence `svg.getBBox()` and therefore the root
viewport.

### 4) HTML label box model drift (`foreignObject` width/height differs)

State diagram nodes use HTML labels (`foreignObject` + `<div>`). If our headless label measurer
produces a different wrapped line count, line height, or vertical padding than the browser
(upstream Mermaid), Dagre receives different node dimensions and the entire layout can drift.

This typically manifests as:

- `parity` mode has 0 mismatches, but `parity-root` has ~0.1–10px root `max-width` / `viewBox` deltas
- `debug-svg-bbox` reports `max_x/max_y` contributors as cluster `<rect class="outer">` frames
- inspecting the emitted SVG shows `foreignObject height` differs for long/wrapped labels

Notes:

- Upstream browser measurements often land on a **1/64px lattice** (sub-pixel rounding). For parity,
  `merman` rounds certain HTML-like label widths to `1/64px` before feeding Dagre.
- Some edge-case labels still require fixture-derived overrides (see
  `crates/merman-render/src/generated/state_text_overrides_11_12_2.rs`) to match the pinned upstream
  baselines exactly.

### 5) Edge label positioning drift (`positionEdgeLabel` + `updatedPath`)

Mermaid positions edge labels using a mix of Dagre's `edge.x/y` and a post-render recalculation
(`utils.calcLabelPosition(...)`), depending on whether the path is considered “updated”.

In Mermaid `@11.12.2` this `updatedPath` mode is enabled when:

- cluster cutting happened (`toCluster` / `fromCluster`)
- or the `isLabelCoordinateInPath(...)` heuristic decides the label anchor point is not present in
  the emitted `d` string (common with `curveBasis` control points)

If `merman` recomputes label centers unconditionally (or never recomputes them), the HTML
`foreignObject` bboxes can become the root `max-width`/`viewBox` extrema, causing parity-root drift.

## Debug Workflow

### A) Identify the root viewport deltas

Run the state DOM compare in root mode:

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

### B) Find which element drives `min/max`

Use the bbox inspector on both SVGs:

```sh
cargo run -p xtask -- debug-svg-bbox --svg fixtures/upstream-svgs/state/<name>.svg --padding 8
cargo run -p xtask -- debug-svg-bbox --svg target/compare/state/<name>.svg --padding 8
```

If `min_x/max_x/max_y` contributors are `<path ... data-points="...">`, proceed to (C).

### C) Compare `data-points` directly

Decode and print the `data-points` for a specific element id:

```sh
cargo run -p xtask -- debug-svg-data-points --svg fixtures/upstream-svgs/state/<name>.svg --id edge1
```

To diff the same id between upstream and local:

```sh
cargo run -p xtask -- debug-svg-data-points --svg fixtures/upstream-svgs/state/<name>.svg --other target/compare/state/<name>.svg --id edge1
```

If the points differ, the root cause is upstream dagre parity (dugong ordering/numerics or graph
construction).

### D) Validate Dagre parity for a nested cluster (JS vs Rust)

If you suspect the drift is caused by the recursive cluster extraction pass (not Dagre itself),
compare the Rust layout output with a JS Dagre run for the same extracted cluster graph:

```sh
cargo run -p xtask -- compare-dagre-layout --diagram state --fixture <fixture_name> --cluster <cluster_id>
```

If JS and Rust match (max deltas ~0), then the remaining root viewport mismatch is almost always
driven by **input graph construction** (node/edge sizes, label measurement, insertion order) rather
than the layout solver.

## Current failing fixtures (parity-root)

Below are the current `compare-state-svgs` `parity-root` deltas (local - upstream) at
`--dom-decimals 3`.

| Fixture | Δmax-width (px) | ΔviewBox.w | ΔviewBox.h |
|---|---:|---:|---:|
| `upstream_stateDiagram_multiple_recursive_state_definitions_spec` | -0.453 | -0.453125 | +0.000 |
| `upstream_stateDiagram_recursive_state_definitions_spec` | -0.453 | -0.453125 | +0.000 |
| `upstream_stateDiagram_v2_note_statements_spec` | +0.433 | +0.433594 | +0.000 |
| `upstream_stateDiagram_note_statements_spec` | +0.433 | +0.433594 | +0.000 |
| `upstream_stateDiagram_v2_concurrent_state_spec` | +0.220 | +0.218750 | +0.000 |
| `upstream_stateDiagram_concurrent_state_spec` | +0.220 | +0.218750 | +0.000 |
| `upstream_stateDiagram_v2_frontmatter_title_docs` | -0.182 | -0.181641 | +0.000 |
| `upstream_stateDiagram_concurrent_state_minimal_spec` | +0.094 | +0.093750 | +0.000 |
| `upstream_stateDiagram_state_definition_separation_spec` | +0.071 | +0.070312 | +0.000 |
| `upstream_stateDiagram_handle_as_in_state_names_spec` | +0.015 | +0.015625 | +0.000 |
| `upstream_stateDiagram_v2_state_definition_with_quotes_spec` | +0.004 | +0.003937 | -1.450 |
| `upstream_stateDiagram_state_definition_with_quotes_spec` | +0.004 | +0.003937 | -1.450 |
| `upstream_stateDiagram_v2_composite_self_link_spec` | +0.000 | +0.000000 | -0.050 |
