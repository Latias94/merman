# Flowchart Render Config And ViewBox Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`flowchart/svg_emit.rs` still mixed SVG orchestration with two separate preparation concerns:
effective render configuration and final content/viewBox bounds. That made the entry point harder to
audit because theme defaults, label mode selection, rough-shape bbox approximation, edge curve bbox
union, and title bbox merging all lived beside the final string emission.

## Target State

`flowchart/svg_emit.rs` coordinates the render. Dedicated modules own the preparation work:

- `flowchart/render_config.rs` prepares font, label, theme, spacing, edge defaults, and text style
  values.
- `flowchart/viewbox.rs` prepares rendered content bounds and final viewBox bounds, including edge
  curve bbox union and diagram title bbox merging.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/render_config.rs`.
- Add `crates/merman-render/src/svg/parity/flowchart/viewbox.rs`.
- Move render configuration preparation out of `svg_emit.rs`.
- Move rendered content bounds and final viewBox preparation out of `svg_emit.rs`.
- Preserve strict SVG DOM output behavior.

## Out Of Scope

- Changing Mermaid parity heuristics or generated SVG structure.
- Reworking node-shape bbox formulas.
- Reworking edge path geometry.
- Performance benchmarking.

## Closeout Condition

- `svg_emit.rs` delegates configuration and viewBox/content-bounds preparation to dedicated
  modules.
- Flowchart DOM parity and package gates pass.
- Evidence records the current local machine and notes that historical benchmark data may have been
  collected on different hardware.
