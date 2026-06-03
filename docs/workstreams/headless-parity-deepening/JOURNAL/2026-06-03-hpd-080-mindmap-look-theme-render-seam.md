# HPD-080 Mindmap Look/Theme Render Seam

Date: 2026-06-03

## Source Finding

Pinned Mermaid 11.15 Mindmap keeps `look` as parser/layout data, not just as stylesheet state:
`mindmapDb.ts` copies `conf.look` into node and edge layout data, and its default node shape uses
`rounded` for `redux*` themes. `styles.ts` then has visible `[data-look="neo"]` node, edge, root,
drop-shadow, and gradient branches, while `mindmapRenderer.ts` inserts a scoped gradient `<defs>`
entry when `useGradient`, `gradientStart`, and `gradientStop` are present.

Local Mindmap had the seam split:

- compatibility JSON and typed render data hardcoded node/edge `look` to `"default"`;
- default semantic data therefore disagreed with Mermaid's configured default `"classic"`;
- redux default node shape still used `defaultMindmapNode`;
- SVG output had no `data-look` surface for the source-backed `neo` CSS branch.

## Implementation

- Threaded `MermaidConfig` into Mindmap node/edge data projection.
- Projected configured `look` into compatibility JSON and typed render models.
- Preserved Mermaid's default `classic` semantic value in snapshots.
- Restored redux default-shape behavior for unspecified Mindmap node types.
- Emitted `data-look="neo"` only for current `neo` node/edge SVG DOM, preserving default/classic
  structural parity.
- Added Mermaid 11.15 `neo` node/root/edge/drop-shadow/gradient CSS and scoped gradient defs.
- Narrowed golden changes to Mindmap only; recursive JSON comparison showed only
  `model.nodes[].look` and `model.edges[].look` changing from `"default"` to `"classic"`.

## Verification

- `cargo nextest run -p merman-core mindmap` - passed, `33` tests run.
- `cargo nextest run -p merman-render mindmap` - passed, `9` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap_neo_theme_smoke_counts_data_look_dom_and_neo_css_as_visible` -
  passed, `1` test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `12`
  tests run.
- `cargo nextest run -p merman-core --test snapshots` - passed, `1` fixture-snapshot gate run.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\mindmap_report_parity_after_hpd080_look_theme.md` -
  passed, all Mindmap fixtures matched structurally.
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\mindmap_report_parity_root_after_hpd080_look_theme.md` -
  expected-failed on the existing `4` Mindmap root residual rows.
- `cargo fmt -p merman-core -p merman-render -p merman --check` - passed.
- `git diff --check` - passed.

## Residual

The four Mindmap `parity-root` rows remain measurement/root residuals, not `look` or theme defects.
Do not tune root bounds through Mindmap `neo` CSS or gradient behavior.
