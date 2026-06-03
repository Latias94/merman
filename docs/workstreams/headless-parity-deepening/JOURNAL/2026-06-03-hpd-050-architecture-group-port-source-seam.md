# HPD-050 - Architecture Group Port Source Seam

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous active-residual phase join identified `stress_architecture_group_port_edges_017` as
the clearest remaining Architecture phase seam. The local outer group height is `444.603px`, which
matches the browser probe's `bbAfterSegments.h`, while the upstream final SVG group rect height is
`462.448px`.

This pass audited the local source path before attempting a renderer formula change.

## Outcome

- Confirmed the local Architecture SVG renderer does not consume a final compound group bbox from
  `manatee`.
- `crates/merman-render/src/architecture.rs` writes only leaf service/junction positions from
  `manatee::algo::fcose::layout_indexed(...)` back into `ArchitectureDiagramLayout`.
- `crates/merman-render/src/svg/parity/architecture.rs` then rebuilds group rectangles in SVG
  space from service bounds, junction bounds, and recursively computed child group bounds through
  `GroupRectComputer`.
- `crates/merman-render/src/svg/parity/architecture/viewport.rs` finalizes the root viewport from
  emitted SVG bounds plus the renderer-owned `content_bounds`; `ArchitectureDiagramLayout.bounds`
  is not the active root viewport source for Architecture parity SVG output.
- Pinned Mermaid 11.15 draws Architecture groups directly from Cytoscape final
  `node.boundingBox()` in `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts`.
- A focused `MANATEE_FCOSE_DEBUG_ELES_BBOX=1` run showed the local `run=1` element bbox as
  `(-313.619,-204.551)-(316.619,240.051)`, height `444.603px`, matching both the browser
  `bbAfterSegments` probe and the local outer group height.
- The same row is not a safe one-constant group padding target: local service and child group
  positions are vertically compressed relative to the stored upstream SVG by `8.922571px` on each
  side, producing the full `17.845142px` outer group/root height tail.
- No production renderer, layout, measurement, or xtask behavior changed.

## Source Seam

The seam is now source-backed:

- upstream group rect phase: Cytoscape final `node.boundingBox()` from `svgDraw.ts`;
- local group rect phase: renderer-side `GroupRectComputer` reconstruction from leaf positions and
  child group bounds;
- local layout evidence phase: `manatee` / browser `bbAfterSegments` `eles.boundingBox()` around
  the FCoSE rerun and segment-stage bbox, not the final compound group bbox used by SVG group
  emission.

This explains why the local outer group height equals `bbAfterSegments.h` but not the final
upstream outer group bbox height.

## Verification

- `MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-debug-group-port-eles` -
  passed and printed the local `run=1` total bbox
  `(-313.618759,-204.551469)-(316.618759,240.051469)`.
- Existing probe evidence:
  `target\compare\architecture-fcose-probe-active-residuals-hpd050\stress_architecture_group_port_edges_017.fcose-browser-probe.md`.
- Existing local delta evidence:
  `target\compare\architecture-delta-active-residuals-hpd050-group-size\stress_architecture_group_port_edges_017.md`.

## Residual Boundary

Do not fix `group_port_edges_017` by globally changing group padding, directly exporting
layout-base compound rectangles, or forcing root height from `ArchitectureDiagramLayout.bounds`.
The next production-worthy path needs a phase-specific model that can distinguish:

- layout/relocation `eles.boundingBox()` used around FCoSE reruns,
- final Cytoscape compound `node.boundingBox()` used by `drawGroups(...)`, and
- service/group position propagation for `{group}` edge endpoints.
