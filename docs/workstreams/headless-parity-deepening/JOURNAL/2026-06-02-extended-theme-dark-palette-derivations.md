# HPD-080 Extended Theme Dark Palette Derivations

Date: 2026-06-02

## Problem

The first extended-theme override derivation slice fixed visible keys for light `neo/redux*`
override paths such as Flowchart edge-label backgrounds and shared line/background derived colors.
A follow-up audit against pinned Mermaid 11.15 showed another visible gap in dark extended themes:

- `theme: "neo-dark"` / `redux-dark` / `redux-dark-color` with
  `themeVariables.primaryColor = "#123456"` derives `requirementBackground`, `pie1`, and
  `quadrant1Fill` to `#123456`.
- `redux-dark` and `redux-dark-color` also derive `git0..git7` and `gitInv0..gitInv7` from the
  current source-backed palette.
- User-supplied `gitN` values should also derive `gitInvN` unless the user explicitly supplies the
  inverse key.

The core theme map was missing those derived values. While adding SVG regressions, Pie exposed a
second production issue: Pie slice/legend colors were still generated from a hardcoded default
palette in layout, so even a correctly derived `themeVariables.pie1` could not affect visible slice
fill.

## Change

- Extended `crates/merman-core/src/theme.rs` so the extended-theme derivation seam now handles:
  - HSL color parsing in addition to hex,
  - `neo-dark` / `redux-dark*` primary-color derivation for `requirementBackground`, `pie1`, and
    `quadrant1Fill`,
  - `redux-dark*` GitGraph palette derivation for `git0..git7`,
  - `gitInv0..gitInv7` derivation from either derived or user-explicit `gitN` colors when inverse
    colors are not explicit.
- Updated `crates/merman-render/src/pie.rs` so Pie layout builds its color scale from
  `effective_config.themeVariables.pie1..pie12`, preserving the default palette and color-domain
  behavior when no theme variables are present.
- Added Pie and QuadrantChart render regressions proving `redux-dark` primary-color overrides reach
  visible slice/quadrant fill.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`
- Installed Mermaid `11.15.0` dist probe through
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.core.mjs`.

Focused official-output probe:

- `node --input-type=module -e "... for (const theme of ['neo-dark','redux-dark','redux-dark-color']) { mermaid.initialize({ theme, themeVariables:{ primaryColor:'#123456' }}) ... }"`

## Verification

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test quadrantchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

## Residual Notes

The extended-theme seam still grows only from source-backed visible surfaces. Do not replace it with
fixture-keyed palette constants. If future `neo/redux*` evidence shows another currently emitted
surface missing derived behavior, add that source rule and a render-path regression.
