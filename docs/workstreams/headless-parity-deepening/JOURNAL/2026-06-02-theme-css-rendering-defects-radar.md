# HPD-080 Radar Style Override Visible Rendering Slice

## Context

Continue HPD-080 after ER. Radar has a dedicated Mermaid 11.15 style provider, and the local renderer
already emitted most radar CSS from `themeVariables.radar`. The visible defect was narrower:
top-level `radar` style overrides were parsed into the effective config but not consumed by CSS.

## Source Evidence

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/radar/styles.ts`
- Source behavior:
  `buildRadarStyleOptions(...)` merges `themeVariables.radar` with the top-level `radar` config,
  allowing diagram config to override style defaults.
- Local renderer:
  `crates/merman-render/src/svg/parity/radar.rs`

## Finding

Local `radar_css` read only `themeVariables.radar.*` for axis, graticule, curve, and legend CSS.
This ignored user-provided top-level style overrides such as `radar.axisColor`,
`radar.curveOpacity`, and `radar.graticuleStrokeWidth`, so rendered output could keep stale visual
settings despite the effective config carrying the override.

## Implementation

- Radar style CSS now resolves `radar.<styleKey>` before `themeVariables.radar.<styleKey>`,
  matching Mermaid 11.15's `cleanAndMerge(themeVariables.radar, radar)` ordering.
- Added focused coverage for top-level style override precedence while preserving `cScale*` palette
  usage for radar curves and legend boxes.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render radar_css_honors_top_level_style_overrides`
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`
