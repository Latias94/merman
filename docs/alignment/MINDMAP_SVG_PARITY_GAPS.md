# Mindmap SVG Parity Gaps (mermaid@11.12.2)

Baseline: Mermaid `@11.12.2` (see `repo-ref/REPOS.lock.json`).

This document tracks the remaining gaps for Mindmap SVG output parity against upstream baselines.

## How to reproduce

- Compare (parity-root):
  - `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
- Debug node positions for a single fixture:
  - `cargo run -p xtask -- debug-mindmap-svg-positions --fixture basic`

## Current mismatches (parity-root)

None (as of 2026-02-07).

Mindmap SVG DOM parity-root is currently 0-mismatch for the tracked fixture set, including the
docs-derived example with `::icon(...)` and `<br/>` inside a label:

- `fixtures/mindmap/upstream_docs_example_icons_br.mmd`

## Notes

- Mermaid mindmap uses Cytoscape with `cose-bilkent` (`quality: "proof"`) for layout.
- `manatee` contains a growing COSE-Bilkent port. The remaining work is primarily in matching
  Cytoscape/cose-base iteration behavior and tie-breaking so the final node coordinates (and thus
  root viewport sizing) match upstream exactly.
