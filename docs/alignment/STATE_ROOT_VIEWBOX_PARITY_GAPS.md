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
