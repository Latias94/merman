# HPD-080 - Class Stylesheet Theme Coverage

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Source Check

- Pinned Mermaid 11.15 source: `packages/mermaid/src/diagrams/class/styles.js`.
- The Class style provider emits source-backed rules for class group text, cluster labels/rects,
  node shapes, dividers, class labels, relations, edge terminals, and title text.
- `strokeWidth` is used as a CSS value in both node shape and relation rules. It is not a color and
  can be numeric in `themeVariables`.

## Local Finding

- Local Class CSS consumed some label/relation/note colors, but skipped several rules that apply to
  the current SVG surface.
- Local Class CSS read `strokeWidth` via `theme.color(...)`, which only reads strings. Numeric
  `themeVariables.strokeWidth` was parsed by core but dropped during CSS emission.
- Upstream icon and neo-only rules are still not emitted in this slice because current local Class
  output does not emit `.label-icon`, `data-look="neo"`, or the corresponding support elements.

## Change

- `class_css(...)` now emits the source-backed current-output rules for class groups, clusters,
  node shape selectors, dividers, class labels, relations, edge terminals, and title text.
- `strokeWidth` now uses `SvgTheme::css_value(...)`, preserving numeric and string CSS token
  values.
- Added a render-path regression for numeric `strokeWidth` driving Class node shape and relation
  CSS.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_numeric_stroke_width_theme_css --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

## Residual

- This is a stylesheet/theme emission fix only. Class root-bounds residuals, namespace cluster
  inline styling, icon labels, and neo rendering remain open unless a later source-backed slice
  adds the required local SVG support.
