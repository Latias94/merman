# HPD-080 Extended Theme Override Derivations

Date: 2026-06-02

## Problem

The earlier Mermaid 11.15 extended-theme slice exposed all official theme names and loaded generated
`neo/redux*` snapshots as defaults. That was correct for no-override theme selection, but it left a
real host-theme gap: when a consumer passed base `themeVariables`, local extended themes merged the
user key directly over a static snapshot instead of following Mermaid's
`calculate(overrides)` shape.

Pinned Mermaid source does not treat those themes as immutable snapshots. Each extended theme copies
user overrides into the theme object, runs `updateColors()`, then copies explicit user keys again so
direct overrides win over derived values.

Concrete upstream check:

- `theme: "redux", themeVariables.primaryColor = "#123456"` derives
  `secondaryColor = "hsl(90, 65.3846153846%, 20.3921568627%)"`.
- The same override derives Flowchart edge-label background from that secondary color.
- It does **not** make the Redux Flowchart node fill `#123456`; source `mainBkg` remains the visible
  node-fill driver and defaults to `#ffffff`.

This is a visible rendering issue, not a browser-font or pixel-parity tail, because the derived keys
feed current SVG CSS for Flowchart, Architecture, Requirement, Sequence, State, GitGraph labels, and
shared edge/label surfaces.

## Change

- Kept generated Mermaid 11.15 theme snapshots as the default source of truth for `neo/redux*`
  themes with no user overrides.
- Added a bounded source-backed derivation seam for extended-theme user overrides in
  `crates/merman-core/src/theme.rs`.
- Recomputed only visible derived keys consumed by current renderers:
  - `primaryColor` to `nodeBkg` and `tagLabelBackground`,
  - light extended-theme `primaryColor` to `secondaryColor`,
  - `secondaryColor` to edge/activation/commit/requirement label backgrounds,
  - `background` to line/arrowhead colors,
  - line color to link, Architecture edge, Requirement relation, and State transition colors,
  - `mainBkg` to actor, label-box, C4 person, State background, and label background colors.
- Preserved Mermaid's explicit-override precedence: if the user directly provides a derived key,
  that value wins after derivation.
- Added a Flowchart SVG regression proving the derived Redux secondary color reaches visible
  edge-label CSS while the Redux `mainBkg` default still controls node fill.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/themes/index.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-color.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`

Focused official-output probes used the installed Mermaid `11.15.0` dist:

- `node --input-type=module -e "... mermaid.initialize({ theme:'redux', themeVariables:{ primaryColor:'#123456' }}) ..."`
- `node --input-type=module -e "... mermaid.initialize({ theme:'redux', themeVariables:{ background:'#010203' }}) ..."`
- `node --input-type=module -e "... mermaid.initialize({ theme:'redux', themeVariables:{ mainBkg:'#101112' }}) ..."`

## Verification

- `cargo fmt --check -p merman-core -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

## Residual Notes

This is intentionally not a full hand-port of every extended theme line. The default snapshots still
own no-override defaults. The new derivation seam covers source-backed user override behavior for
keys that current renderers visibly consume. If future fixtures or consumers show remaining
`neo/redux*` override drift in currently emitted surfaces, extend this seam with the specific source
rule rather than replacing it with fixture-keyed constants.
