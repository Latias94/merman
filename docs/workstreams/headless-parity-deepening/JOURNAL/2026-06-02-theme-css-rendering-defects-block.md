# HPD-080 Block Cluster Theme CSS Slice

## Context

Continue HPD-080 after Radar. Block already consumed most Mermaid 11.15 theme variables, but its
composite cluster CSS still emitted raw `clusterBkg` and `clusterBorder` values.

## Source Evidence

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/block/styles.ts`
- Source behavior:
  `.node .cluster` uses `fade(clusterBkg, 0.5)` and `fade(clusterBorder, 0.2)`.
- Local renderer:
  `crates/merman-render/src/svg/parity/block.rs`

## Finding

Local Block composite clusters used fully opaque theme colors. Nested block output could therefore
look visually heavier than Mermaid 11.15 and reduce label readability around composite regions,
especially with custom cluster colors.

## Implementation

- Block cluster fill now uses `css_rgba_fade(clusterBkg, 0.5)` when the configured color is
  parseable.
- Block cluster stroke now uses `css_rgba_fade(clusterBorder, 0.2)` when the configured color is
  parseable.
- Unparseable runtime CSS expressions fall back to the original configured color rather than
  pretending browser-side resolution.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render block_svg_fades_cluster_theme_colors --test block_svg_test`
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`
