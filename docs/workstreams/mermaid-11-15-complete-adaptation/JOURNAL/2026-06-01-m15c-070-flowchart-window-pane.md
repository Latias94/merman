# M15C-070 Flowchart Window Pane Geometry

Date: 2026-06-01

## Context

The leading Flowchart strict-root residual after the bow-tie rectangle slice was
`upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset27_027`:

```mermaid
flowchart
 n0@{ shape: win-pane, label: "win-pane" }
 n1@{ shape: internal-storage, label: "internal-storage" }
 n2@{ shape: window-pane, label: "window-pane" }
```

With root overrides disabled, local output was `15px` narrower and `5px` shorter than the
Mermaid 11.15 baseline.

## Diagnosis

Mermaid 11.15 `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/windowPane.ts`
uses:

- `const rectOffset = 10`
- `totalWidth = bbox.width + paddingX * 2 + rectOffset`
- `totalHeight = bbox.height + paddingY * 2 + rectOffset`
- a shape transform of `translate(rectOffset / 2, rectOffset / 2)`

Local layout, SVG rendering, and edge intersection still used `rectOffset=5`. Since the fixture has
three window-pane aliases in one row, the width delta was exactly `3 * 5px = 15px`; the height delta
was `5px`.

## Change

- Updated Flowchart layout sizing for `win-pane`/`internal-storage`/`window-pane` to add
  `rectOffset=10`.
- Updated the SVG shape renderer to build the path and label offset from the same `rectOffset=10`.
- Updated edge intersection geometry to use the same polygon extents.
- Added window-pane coverage to `flowchart_node_shape_dimensions_follow_mermaid_rules`.

## Evidence

- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset27_027 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: expected failed for remaining alias buckets; alias set 27 disappeared from the mismatch list.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: expected failed with 129 Flowchart strict root-only mismatches, down from 144.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

## Follow-Up

Continue M15C-070 with the remaining Flowchart strict-root residuals: shape-alias `20`, `21`, `12`,
`29`, `38`, unpinned `34`, delay half-rounded rectangle, Unicode/text metrics, markdown subgraph
root size, and shape-family layout clusters.
