# HPD-050 - Architecture Group Content Union Audit

Date: 2026-06-04
Task: HPD-050 layout engine source-backed audit

## Context

The label-phase join left three direct group-width tails active: `batch5_long_titles` `+5px`,
`html_titles` `+5px`, and `unicode` `+3px`. The new probe tables showed that changing final group
expansion alone is not source-backed. This pass audited the local source path that feeds those group
rect widths.

## Source Findings

- Pinned Mermaid 11.15 `svgDraw.ts::drawGroups(...)` draws group rectangles from final Cytoscape
  `node.boundingBox()`, then offsets `x` / `y` by `halfIconSize`; group title text is emitted after
  the rect and does not drive that compound bbox.
- Local `architecture_estimate_service_bounds(...)` separates three phases:
  `emitted_icon_bounds`, `svg_root_bounds`, and `cytoscape_group_child_contribution`.
- Local in-group services feed `GroupRectComputer` through
  `cytoscape_group_child_contribution.union_bounds`.
- Local `GroupRectComputer` unions service, junction, and child-group bounds, then inflates the
  content by `architecture_svg_group_bbox_padding_px(padding)`. With default Architecture padding,
  that is `40 + 2.5 = 42.5px` per side.
- Local root bounds then union the emitted group rect, so direct group-width tails propagate to root
  max-width without needing a separate root-only explanation.

## Debug Evidence

Focused `MERMAN_ARCH_DEBUG_GROUP_RECT` runs on current HEAD show the active direct width tails are
already present in the child content union phase:

- `batch5_long_titles` `pipeline`:
  content `(-194.463,-83.463)-(188.463,214.463)`, pad `42.5`, final local group width
  `467.926`; upstream group width is `462.926`.
- `html_titles` `ui`:
  content `(-129.963,-83.463)-(189.963,214.463)`, pad `42.5`, final local group width
  `404.926`; upstream group width is `399.926`.
- `unicode` `i`:
  content `(-131.911,-83.797)-(175.911,214.797)`, pad `42.5`, final local group width
  `392.822`; upstream group width is `389.822`.

Those numbers match the current local delta reports:

- `batch5_long_titles`: local group `dw=+5`.
- `html_titles`: local group `dw=+5`.
- `unicode`: local group `dw=+3`.

## Decision

Do not change production group padding, root padding, group title root bounds, or final group rect
emission in this slice. The residual source is now narrower: child service label/content bounds
feeding the group content union are still a few pixels wider than the final Cytoscape group rect
phase used by upstream. Prior exact labelWidth experiments improved the focused rows but still left
`+2px` and raised the full Architecture root queue, so a standalone service label lookup remains
rejected.

## Verification

- Source read:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts`
  - `crates/merman-render/src/architecture_metrics.rs`
  - `crates/merman-render/src/svg/parity/architecture.rs`
  - `crates/merman-render/src/svg/parity/architecture/geometry.rs`
  - `crates/merman-render/src/svg/parity/architecture/nodes.rs`
  - `crates/merman-render/src/svg/parity/architecture/viewport.rs`
- `MERMAN_ARCH_DEBUG_GROUP_RECT=pipeline cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_batch5_long_titles_and_punct_076 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed and printed the content/pad/final rows above.
- `MERMAN_ARCH_DEBUG_GROUP_RECT=ui cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_html_titles_and_escapes_041 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed.
- `MERMAN_ARCH_DEBUG_GROUP_RECT=i cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_unicode_and_xml_escapes_019 --out target\compare\architecture-delta-debug-label-phase-grouprect-current` -
  passed.

## Residual Boundary

The next viable production candidate must be scoped to the child service-label/content union phase
and must be verified against the full Architecture root queue. A single constant change to group
padding or root consumption remains rejected.
