# Flowchart Text Style Parity

## Problem

Flowchart labels currently support the main Mermaid text style path, but they do not fully mirror
Mermaid's `isLabelStyle(...)` boundary or browser-computed font-size behavior. This creates drift
for class/style declarations such as `font-style`, `text-decoration`, `letter-spacing`, and
relative `font-size` values.

## Target State

- Flowchart style classification uses the same label-style key set as Mermaid.
- Node, edge, and subgraph label rendering pass supported label styles through to the same label
  surfaces Mermaid styles.
- Layout measurement consumes text styles that affect label dimensions before Dagre layout.
- Remaining browser-CSS measurement gaps are explicit, test-covered, and split into follow-up
  tasks instead of hidden in ad hoc overrides.

## Scope

- `crates/merman-render/src/flowchart/*`
- `crates/merman-render/src/svg/parity/flowchart/*`
- Shared SVG parity style helpers where Mermaid owns a shared label-style boundary.
- Focused flowchart layout/SVG tests.

## Non-Goals

- Introducing a browser, DOM, or full CSS layout engine.
- Changing host styling postprocessor behavior.
- Replacing vendored font metrics.
- Solving all diagram families in this lane.

## Architecture Direction

The style boundary should be deeper than a local allowlist in each renderer. Mermaid treats label
style classification as a rendering-util concern, so merman should keep a shared helper for that
classification and let flowchart-specific code decide which styles also affect measurement.

The first slice supports full label-style routing and relative `font-size` measurement. The second
slice keeps whole-label `font-style` measurement flowchart-local by passing the effective CSS value
into label metrics instead of expanding the global `TextStyle` constructor surface. Later slices can
add measured effects for `letter-spacing`, `word-spacing`, and line-height only with fixture
evidence.

## Validation

- `cargo fmt --check`
- `cargo nextest run -p merman-render --test flowchart_svg_test --test flowchart_layout_test`
- Targeted strict SVG/root parity commands after additional measurement-affecting slices.
