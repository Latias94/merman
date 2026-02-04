# Mindmap SVG Parity Gaps (mermaid@11.12.2)

Baseline: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

This document tracks the remaining gaps for Mindmap SVG output parity against upstream baselines.

## How to reproduce

- Compare (parity-root):
  - `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
- Debug node positions for a single fixture:
  - `cargo run -p xtask -- debug-mindmap-svg-positions --fixture basic`

## Current mismatches (parity-root)

The remaining Mindmap parity-root mismatches are dominated by root viewport attributes (`viewBox` and
`style="max-width: ...px"`), which depend on the final COSE-Bilkent placements and our headless
measurement of node sizes.

As of 2026-02-04, the following fixtures are still mismatching in `parity-root` mode:

- `basic`
- `upstream_decorations_and_descriptions`
- `upstream_hierarchy_nodes`
- `upstream_node_types`
- `upstream_root_type_bang`
- `upstream_root_type_cloud`
- `upstream_shaped_root_without_id`
- `upstream_whitespace_and_comments`

## Notes

- Mermaid mindmap uses Cytoscape with `cose-bilkent` (`quality: "proof"`) for layout.
- `manatee` contains a growing COSE-Bilkent port. The remaining work is primarily in matching
  Cytoscape/cose-base iteration behavior and tie-breaking so the final node coordinates (and thus
  root viewport sizing) match upstream exactly.

