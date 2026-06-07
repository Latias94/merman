# HPD-050 Flowchart Deep-Subgraph Panic Surface

Date: 2026-06-07

## Context

After the State deep-composite cleanup, Flowchart remained a public-input tree-shaped family worth
checking because `flowchart TB` plus nested `subgraph ... end` syntax can build deep cluster
chains. Earlier Flowchart hardening covered helper traversals discovered through the Zed PR 58325
audit, but it did not prove the ordinary parse -> layout -> SVG path for a deep public input.

## Red Signal

A `1,200`-level nested subgraph chain produced this phase split:

- `parse_diagram_for_render_model_sync(...)` passed.
- `layout_parsed(...)` aborted with stack overflow.
- The initial SVG root traversal cleanup was not sufficient because the red point was still in
  layout.

## Fix

- Converted Flowchart extracted cluster placement from recursive `place_graph(...)` calls to an
  explicit frame stack.
- Converted fallback compound subtree rect collection to an explicit stack.
- Converted final cluster rectangle postorder computation from recursive `compute_cluster_rect(...)`
  calls to explicit enter/exit frames with the existing cycle guard semantics.
- Converted nested Flowchart SVG root output from recursive `render_flowchart_root(...)` calls to
  explicit render frames while preserving nested `.root` group order.
- Added public-path regressions for deep Flowchart parse-for-render-model, layout, and SVG output.

## Verification

- `cargo nextest run -p merman-render flowchart_parse_for_render_model_handles_deep_subgraph_chain`
  - passed, `1` test run.
- `cargo nextest run -p merman-render flowchart_layout_handles_deep_subgraph_chain`
  - failed before the fix with stack overflow; passed after the non-recursive layout changes.
- `cargo nextest run -p merman-render flowchart_svg_handles_deep_subgraph_chain`
  - passed after the SVG root traversal and current DOM id assertion.
- `cargo nextest run -p merman-render flowchart`
  - passed, `106` tests run.
- `cargo fmt --check -p merman-render`
  - passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
  - passed.
- `git diff --check`
  - passed.

## Boundary

No SVG baseline, root override, Architecture formula, or Flowchart residual tuning changed. This is
release-boundary hardening for accepted deep Flowchart subgraph input, not a claim that Flowchart
`parity-root` max-width diagnostics are closed.
