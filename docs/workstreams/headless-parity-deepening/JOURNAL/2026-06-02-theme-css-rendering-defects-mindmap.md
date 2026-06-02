# HPD-080 Mindmap Theme CSS Visible Rendering Slice

Date: 2026-06-02

## Scope

Continue HPD-080's visible-rendering-defect audit, prioritizing supported diagrams whose SVG DOM can
compare cleanly while text or semantic colors become wrong under Mermaid 11.15 themes.

## Source Checked

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/mindmap/styles.ts`
- Local renderer:
  `crates/merman-render/src/svg/parity/mindmap.rs`

## Finding

The local Mindmap CSS still treated the Mermaid default palette as fixed output. It did not read
`themeVariables.cScale*`, `cScaleLabel*`, `cScaleInv*`, `git0`, `gitBranchLabel0`, `nodeBorder`,
`THEME_COLOR_LIMIT`, or `look` while emitting section/root CSS.

The visible defect is especially sharp for labels: local Mindmap renders XHTML labels as
`<span class="nodeLabel">...`, while the old CSS only emitted one non-root span color rule
(`.section-2 span`) plus a fixed white root span. Upstream Mermaid 11.15 emits a `span` color rule
for every generated section and uses `gitBranchLabel0` / redux `nodeBorder` for root spans.

## Change

- Mindmap CSS now uses config-aware Mermaid 11.15 section colors:
  - `THEME_COLOR_LIMIT`
  - `cScale*`
  - `cScaleLabel*`
  - `cScaleInv*`
- Root colors now use `git0`, `gitBranchLabel0`, and redux `nodeBorder`.
- Section spans and node icons now follow `cScaleLabel*`, so XHTML labels are readable under custom
  themes.
- Edge-depth stroke widths now use Mermaid 11.15's `look === "neo"` formula.

## Boundary

I did not emit the upstream `[data-look="neo"]` gradient/drop-shadow selector block. Current local
Mindmap SVG nodes do not emit `data-look`, so those rules would be inert and would overstate parity.
This keeps the slice focused on CSS that actually affects current headless output.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render mindmap_css_honors_mermaid_11_15_theme_sections`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`

Mindmap structural SVG parity stayed green. This slice fixes visible theme/readability behavior; it
does not claim root-bounds closure.
