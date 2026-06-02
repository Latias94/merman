# HPD-080 - Theme Renderability Smoke And Flowchart Node Text Color

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

The Zed integration feedback showed that host palette rewriting should remain a host boundary, but
it also raised a broader question: can common custom Mermaid theme variables make supported
diagrams visibly readable through the public headless render API?

This slice added a small cross-diagram dark-theme smoke for representative supported diagrams and
used it to find real renderability defects instead of chasing pixel parity.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked source files:

- `packages/mermaid/src/diagrams/flowchart/styles.ts`
- `packages/mermaid/src/diagrams/kanban/kanbanRenderer.ts`
- `packages/mermaid/src/rendering-util/rendering-elements/clusters.js`
- `packages/mermaid/src/rendering-util/rendering-elements/shapes/util.ts`
- `packages/mermaid/src/diagrams/kanban/styles.ts`

Important source-backed findings:

- Flowchart labels use `nodeTextColor || textColor` in Mermaid 11.15.
- Kanban upstream SVG fixtures include `class="cluster undefined ..."` and
  `class="node undefined"` because the shared render helpers concatenate missing `cssClasses`.
  That placeholder class is upstream behavior, not a local render breakage.
- Kanban priority metadata is represented visually through the priority side line, not as rendered
  priority text.

## Outcome

- Flowchart CSS now reads `themeVariables.nodeTextColor` and applies it to `.label` and
  `.label text, span`, while root text color continues to use `themeVariables.textColor`.
- Added a renderer regression in `crates/merman-render/tests/flowchart_svg_test.rs` for
  `nodeTextColor`.
- Added a public API smoke in `crates/merman/tests/theme_renderability_smoke.rs` for dark/custom
  theme signals across Flowchart, Sequence, Kanban, GitGraph, and XYChart.
- The smoke keeps `NaN` as a hard failure but permits the upstream Kanban placeholder class shape
  after confirming it exists in Mermaid 11.15 fixtures.

## Verification

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render flowchart_svg_honors_node_text_color_theme_variable --test flowchart_svg_test`
- `cargo test -p merman-render --test flowchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo fmt --check -p merman-render -p merman`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual

This is a theme contract fix, not a browser-metric or pixel-parity claim. The new smoke should stay
small and semantic: labels, no broken geometry, and source-backed theme colors in final SVG output.
Do not broaden it into a screenshot or exact color-compositing gate unless a real consumer failure
requires that level.
