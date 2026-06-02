# HPD-080 Flowchart Stroke Theme CSS

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 commit:
  `41646dfd43ac83f001b03c70605feb036afae46d`
- Audited source:
  `packages/mermaid/src/diagrams/flowchart/styles.ts`
- Mermaid 11.15 Flowchart emits node shape stroke width as
  `stroke-width: ${options.strokeWidth ?? 1}px` and edge path stroke width as
  `stroke-width: ${options.strokeWidth ?? 2}px`.
- Mermaid 11.15 theme defaults define numeric `strokeWidth` values, including `1` for default
  themes and `2` for `neo` / redux-style themes.

## Change

- `flowchart_css(...)` now reads `themeVariables.strokeWidth` through `SvgTheme::css_value(...)`.
- Node shape and `.edgePath .path` CSS now use that value with Mermaid's Flowchart `px` suffix
  behavior.
- Added a full parse/layout/render regression for numeric `themeVariables.strokeWidth`.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render flowchart_svg_honors_mermaid_11_15_numeric_stroke_width_theme --test flowchart_svg_test`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Adjacent Finding

During the same audit, a proposed Class stroke-width slice was deferred because full Class
structural comparison exposed an unrelated namespace semantic/render issue:

- `stress_class_comments_inside_namespaces_024`
- `stress_class_nested_namespaces_many_levels_021`
- `stress_class_unicode_namespace_mix_017`

The first two differ around namespace-qualified relation `data-id` values such as
`id_Outer.Foo_Bar_1` versus `id_Foo_Bar_1`; the unicode namespace fixture also differs in emitted
children around nested namespace structure. This should become a source-backed Class namespace /
qualified-id slice rather than a CSS-only theme patch.
