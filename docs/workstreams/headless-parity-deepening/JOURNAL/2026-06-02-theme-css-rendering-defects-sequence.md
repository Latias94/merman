# HPD-080 Sequence Theme CSS Rendering Defect Slice

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 source:
  `repo-ref/mermaid@41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/sequence/styles.js`
- Local implementation:
  `crates/merman-render/src/svg/parity/sequence/css.rs`
  `crates/merman-render/src/svg/parity/sequence/render.rs`

## Diagnosis

Sequence structural SVG parity could stay green while the emitted stylesheet remained visually
stale. The local Sequence CSS still mirrored an older hardcoded style provider and ignored
effective Mermaid theme variables for actor boxes, actor text, lifelines, signal lines, message
text, label boxes, loop/section titles, notes, activation bars, marker/error colors, rect node
border/drop-shadow, and `noteFontWeight`.

This is a functional renderability defect for dark and custom themes: the SVG structure is valid,
but text and semantic color cues can become wrong or low-contrast.

## Implementation

- Changed `sequence_css(...)` to accept `effective_config` and read through `SvgTheme`.
- Mapped the source-backed Mermaid 11.15 Sequence style options:
  `actorBorder`, `actorBkg`, `strokeWidth`, `dropShadow`, `actorTextColor`, `actorLineColor`,
  `signalColor`, `sequenceNumberColor`, `signalTextColor`, `labelBoxBorderColor`,
  `labelBoxBkgColor`, `labelTextColor`, `loopTextColor`, `noteBorderColor`, `noteBkgColor`,
  `noteTextColor`, `noteFontWeight`, `activationBkgColor`, `activationBorderColor`,
  `nodeBorder`, plus base `textColor`, `lineColor`, `errorBkgColor`, `errorTextColor`, and
  `fontFamily`.
- Added `SvgTheme::optional_value(...)` so numeric/string non-color CSS values are not read through
  `optional_color(...)`.
- Added a render-path regression test proving init `themeVariables` reach the emitted Sequence
  `<style>` block.

## Boundary

The pinned upstream style provider contains `rect.actor.outer-path[data-look="neo"]` and
`rect.note[data-look="neo"]` rules. Current local Sequence SVG output does not emit the required
`data-look` / `outer-path` attributes on those elements, so this slice does not emit inert
selectors. That keeps HPD-080 focused on visible renderability instead of false CSS parity.

This slice does not address the known Sequence generated-measurement/root-width residuals.

## Validation

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render sequence_css_uses_configured_font_size`
- `cargo test -p merman-render sequence_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render sequence_svg_honors_mermaid_11_15_theme_css_options --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Next

Continue HPD-080 by scanning remaining supported diagrams for missing Mermaid 11.15 style provider
coverage, unreadable labels, blank/dark blocks, and parsed theme/config values that never reach the
emitted SVG/CSS.
