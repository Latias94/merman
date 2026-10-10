# Native text shadow containment

Date: 2026-09-18. Base: `0bf29d0f9`.
Plan: [theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md), U8 preparation and U5 regression.
Status: scoped verification and independent source review complete; U8 remains open.

## Why this boundary changed

Ordinary SVG text glow must not depend on an embedded-font catalog. The renderer can preserve
its existing measurement policy and let the final SVG host resolve the text. Native export already
owns a resolved usvg tree and font database, so that is the appropriate place to observe actual
native glyph paint without switching the renderer's layout metrics or loading extra dependencies.
The [updated investigation](2026-09-18-xychart-text-glow-design.md) corrects the earlier proposal
that treated prepared native labels as a prerequisite for ordinary SVG effects.

The existing native filter receipt compared raw SVG definitions with the resolved filter graph
and region. Two new tests failed against that implementation: a valid filter whose region clips
the glyph glow, and a sufficient filter whose transformed glow falls outside the root viewport.
Matching filter parameters did not prove visible containment.

## Implementation

The existing native tree walk now summarizes text outline bounds once per group. usvg's
`Text::stroke_bounding_box()` comes from flattened glyph outlines; its separate `bounding_box()`
follows SVG text metrics and remains the correct input for object-bounding-box filter mapping.
The new containment check uses the former while retaining the existing mapping check.

Filter rectangle equality now uses the same two-step native rectangle normalization as usvg: raw
xywh first, followed by object-bounding-box scaling where applicable. Comparing raw widths to
widths recovered from rounded right/bottom coordinates incorrectly rejected an exact glyph-fit
region. The corrected comparison remains exact after representation normalization.

The renderer and exporter share the existing ordered four-sigma shadow envelope calculation.
SourceGraphic resets to source bounds; Previous accumulates prior shadow outsets. The native
receipt projection also handles unequal horizontal and vertical standard deviations.

A text effect must fit its local filter rectangle and the actual export viewport after transforms.
PNG/JPEG pass the rectangle selected by `raster_geometry_for_svg()`, including its original-coordinate
content crop when viewBox is absent. PDF passes its SVG drawing viewport. No second root-viewBox
translation is applied. Floating-point slack is limited per compared edge; a large canvas must
not hide small clipping at its origin. Unknown ancestor clipping/masking or nested filtered text
prevents certification. Empty filter groups cannot certify discarded text. Shape-only filters
retain their previous containment behavior.

The check observes the existing tree, performs no shaping, adds no retained per-label strings,
and does not run for ordinary SVG output. Its traversal is linear in the visited tree; this is a
source-complexity statement, not a measured latency or memory claim. Existing system/host font
admission remains separate; a successful filter check does not make system fonts Portable.

## Verification

- Initial focused red run: 0/2 passed, both failed for the intended containment assertions.
- First implementation run: compile error from treating `Size::to_rect()` as a Rect rather than
  Option; fixed while connecting the actual target-selected viewport.
- Focused regression: 22/23 passed; empty text exposed the retained empty-filter-group case.
- After that correction: 23/23 passed.
- The added outline-versus-font-metrics positive exposed the rectangle normalization issue:
  41/42 export tests passed before the correction, then 42/42 passed. A further fractional
  object-bounding-box regression covers both normalization steps and changed-region rejection.
- Renderer Release library plus Flowchart effects and XY SVG regression, with
  `embedded-fonts,layout-cytoscape`: 2,788 passed, two skipped.
- Final exporter Release library (`png,pdf`), including fractional-region regression: 43/43 passed.
- Public facade Release tests (`svg,png,pdf,embedded-fonts,layout-cytoscape`): 13/13 passed across
  `theme_flowchart_edge_effects`, `theme_composed_effects`, `theme_xychart_text`, and
  `theme_xychart_series`, including actual PNG/PDF exports and existing prepared native labels.
- Exporter Clippy (`png,pdf`): completed successfully; 168 renderer and 19 exporter warnings
  remain in existing code. No diagnostic targets the new containment implementation.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Independent source review: complete, no remaining findings. Nine review lenses and the three
  simplification rubrics ran in one independent Codex context after an actual agent-capacity
  failure; this is not nine independent reviews. The reviewer did not run Cargo.
  Receipt: `/tmp/compound-engineering-501/ce-code-review/20260918-183105-259f169b/review.json`.
  The review confirms the actual viewport and two-step rectangle normalization fixes. The four
  Rust files match the completed review snapshot; final runtime checks above were run by the parent.

Logs: `/tmp/merman-text-shadow-native-containment-red.log`,
`/tmp/merman-text-shadow-native-containment-green.log`,
`/tmp/merman-native-text-containment-export-final.log`,
`/tmp/merman-native-text-containment-render-final.log`,
`/tmp/merman-native-text-containment-facade-final.log`, and
`/tmp/merman-native-text-containment-clippy.log`.

## Remaining work

This increment does not implement XY text filter emission or add text glow to public Cyberpunk.
It establishes the native observation seam required to implement ordinary host-dependent text
painting honestly. U6/U7/U8 complete scenes, U9 qualification, U10 measured costs and C7a remain
open. No full workspace or all-platform rebuild is claimed.

The separate Web background tests already landed in `9d8438ca9`; the older admission record now
links to their original-source evidence rather than continuing to list those assertions as absent.
That correction does not claim those browser artifacts were rebuilt at this HEAD.
