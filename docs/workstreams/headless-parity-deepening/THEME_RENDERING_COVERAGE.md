# HPD-080 Theme Rendering Coverage

Date: 2026-06-02
Baseline: Mermaid `11.15.0`
Pinned source commit: `41646dfd43ac83f001b03c70605feb036afae46d`

This ledger tracks the theme and style-provider side of HPD-080. Its purpose is to prevent two
failure modes:

- treating structurally valid but unreadable SVGs as acceptable,
- treating host-specific palette rewriting as a default merman parity requirement.

## Consumer Model

| Consumer need | merman stance | Current support |
| --- | --- | --- |
| Mermaid-compatible default SVG readability | Product requirement | Diagram-specific source-backed CSS is emitted for the implemented matrix where Mermaid 11.15 has a provider and local SVG elements can consume the rules. |
| Official Mermaid theme names | Product requirement | Core, bindings, and `@merman/web` expose all 11 Mermaid 11.15 themes: `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, and `redux-dark-color`. Extended theme defaults use generated upstream snapshots; source-backed user override derivation now covers visible current-renderer keys. |
| Custom Mermaid theme variables | Product requirement | Renderers pass `effective_config` into CSS or inline style generation. Rust and shared binding consumers can pass external Mermaid defaults through site config; non-color CSS tokens use string/number-aware paths such as `SvgTheme::css_value(...)` where needed. |
| Browser-free raster-safe output | Product requirement | `SvgPipeline::resvg_safe()` inserts SVG text fallbacks for `<foreignObject>` labels, strips unsupported foreignObject content, and sanitizes CSS/attributes for resvg. |
| Host palette replacement, such as Zed markdown preview colors | Host integration boundary | Hosts can pass Mermaid config, compose Rust postprocessors, or use binding `svg.scoped_css`. merman should not inject Zed-specific edge-label, tag-label, or background colors by default. |
| Native/fallback duplicate label cleanup | Optional host pipeline feature | Rust users can compose `DropNativeDuplicateFallbacksPostprocessor`; binding users can set `svg.drop_native_duplicate_fallbacks=true`. Both paths drop only fallback groups whose text duplicates native SVG `<text>`, preserving fallback-only labels. |
| Exact browser font fallback and glyph rasterization | Host/rendering boundary | merman can expose configured font families and deterministic measurement approximations, but it should not claim browser font fallback parity in a headless renderer. |
| Root background replacement | Supported and opt-in host policy | Mermaid 11.15 `setupGraphViewbox` emits `max-width` but not `background-color`; local parity output keeps fixture/capture-compatible white backgrounds. Rust hosts can use `RootBackgroundPostprocessor`; binding hosts can set `svg.root_background_color`. Defaults stay unchanged. |
| Huge texture caps for previews | Host boundary with possible helper API | Zed/GPUI and similar hosts must cap preview textures. A future merman raster helper may expose explicit max-pixmap policy. |

## Implemented Matrix Coverage

Status terms:

- `Covered`: source-backed theme rules that apply to current local output are implemented.
- `Inline`: Mermaid does not expose a CSS provider for the diagram, or visible theme behavior is
  primarily inline renderer configuration.
- `Deferred`: upstream has rules that rely on elements, attributes, gradients, filters, or browser
  behavior that local headless output does not currently emit.
- `Boundary`: not a default merman SVG requirement.

| Diagram | Mermaid 11.15 style source | Local status | Boundary notes |
| --- | --- | --- | --- |
| Architecture | `architecture/architectureStyles.ts` | Covered | Edge, arrow, and group-border theme variables are emitted. Layout/root residuals remain separate HPD-050 work. |
| Block | `block/styles.ts` | Covered | Composite cluster fade colors are source-backed. Continue fixing only rules that apply to emitted Block SVG. |
| C4 | `c4/styles.js` | Covered, narrow | Upstream provider only emits `.person`, while current C4 output uses `person-man` groups rather than `class="person"`. Public smoke coverage now treats `.person` as provider-only evidence and proves visible colors through inline C4 config plus `UpdateElementStyle` / `UpdateRelStyle`; do not promote unrelated theme keys into C4 shapes. |
| Class | `class/styles.js` | Covered | Current-output node shape, divider, cluster, class-label, edge-terminal, relation, and note colors are covered. Icon and neo-only rules remain deferred until local output emits the required support elements/attributes. |
| ER | `er/styles.ts` | Covered for current visible DOM, with legacy provider-rule caveats | Entity, XHTML label, relationship, marker, edge-label, error, and current neo stroke rules are covered. Public smoke now counts direct node/edge colors, `nodeTextColor` through current XHTML labels, `tertiaryColor` through the current `.labelBkg` rgba fade, and `edgeLabelBackground` through current XHTML edge labels. Direct `.relationshipLabelBox` fills and native `.edgeLabel .label text` rules are provider/native-text coverage for the current sample, not visible signals. `data-color-id` and unsupported neo label-background selectors remain deferred. |
| Flowchart | `flowchart/styles.ts` | Covered for current HPD-080 gaps | `strokeWidth` reaches node and edge-path CSS, and `nodeTextColor || textColor` now drives Flowchart labels. Larger Flowchart CSS parity should stay fixture/source driven because many rules are tied to emitted shape attributes and renderer-specific DOM. |
| Gantt | `gantt/styles.js` | Covered | Section, grid, today marker, task state, outside label contrast, marker, and title theme variables are emitted. Public smoke coverage now uses matching ordinary-task, outside-label, and done-task DOM before counting state-specific colors as visible. |
| GitGraph | `git/styles.js` | Covered | Branch label, commit, arrow, merge, reverse, highlight, and label color rules are emitted for classic/default branch theme variables and for Mermaid 11.15 `neo` / `redux*` color-generation theme branches. Neo gradient defs are emitted when the active theme variables require them. |
| Info | none | Inline/shared base | No diagram-specific Mermaid provider in 11.15. Shared base CSS is sufficient unless a visible fixture proves otherwise. |
| Journey | `user-journey/styles.js` | Covered for current visible DOM, with inert provider-rule caveats | Current visible Journey DOM consumes text/line color through the generic `line` / label / legend rules, face color through `.face`, task/section colors through `task-type-*` / `section-type-*`, and actor colors through `actor-*`. Mermaid 11.15 also emits inherited Flowchart-like rules for `.edgePath .path`, `.flowchart-link`, `.edgeLabel`, `.cluster text`, `.node ...`, and `.arrowheadPath`, but current Journey output does not emit matching DOM, so `lineColor`, `edgeLabelBackground`, `mainBkg`, `nodeBorder`, `titleColor` via `.cluster text`, and `arrowheadColor` are tracked as upstream-inert provider rules rather than public renderability signals. |
| Kanban | `kanban/styles.ts` | Covered | Section, ticket, icon, and label theme CSS fixes the dark-card/hidden-label defect class. |
| Mindmap | `mindmap/styles.ts` | Covered with deferred rules | Section/root/icon/span colors are covered. Public smoke now counts root `git0` fill, redux `nodeBorder` through the current root XHTML `span`, and child `cScale1` / `cScaleLabel1` / `cScaleInv1` through matching `.section-0` DOM. Compact `cScale0` / `cScaleLabel0` root-section rules and `gitBranchLabel0` native-text CSS are provider/overridden or native-text-only for current output, not visible signals. `data-look` gradient/drop-shadow rules are deferred until local output emits matching attributes/defs. |
| Packet | `packet/styles.ts` | Covered | `packet.*` style options drive byte, label, title, and block CSS; public smoke now requires matching `.packetByte`, `.packetLabel`, `.packetTitle`, and `.packetBlock` DOM before counting those colors as visible. |
| Pie | `pie/pieStyles.ts` | Covered | Stroke, opacity, title, slice, legend, font family, text colors, and slice/legend palette colors read Mermaid 11.15 theme variables. |
| QuadrantChart | `quadrant-chart/quadrantDiagram.ts` uses `styles: () => ''` | Inline, render-path covered | Theme behavior is inline through quadrant chart config, classDef, and point styles. No CSS provider should be invented. Mermaid 11.15's default `quadrantPointFill` currently expands to `hsl(...NaN%)`; merman intentionally emits a valid derived default while preserving valid explicit point-color overrides. |
| Radar | `radar/styles.ts` | Covered | Top-level `radar.*` overrides are resolved before `themeVariables.radar.*`, matching Mermaid's clean-and-merge behavior. |
| Requirement | `requirement/styles.js` | Covered for current visible DOM, with legacy provider-rule caveats | Relationship lines/markers, edge-label backgrounds, and `look: neo` node/divider borders are covered for current output. Mermaid 11.15 also emits legacy `.reqBox`, `.reqTitle`, `.reqLabel`, `.reqLabelBox`, and `.relationshipLabel` rules that ordinary Requirement DOM does not consume, so those colors are provider coverage rather than public visible-signal coverage. `data-color-id` palette rules remain deferred until local output emits matching attributes. |
| Sankey | `sankey/styles.js` | Covered | Label, outlined-label background, node, and link style options are emitted; public smoke now requires outlined `.sankey-label-bg` / `.sankey-label-fg`, node rect fills, and link groups before counting those colors as visible. |
| Sequence | `sequence/styles.js` | Covered with deferred rules | Actor, lifeline, signal, label, loop/section, note, activation, marker/error, and rect-node theme variables are covered. Neo-only selectors remain deferred without matching local DOM. |
| State | `state/styles.js` | Covered with deferred rules | State node, cluster, transition, label, note, marker, start/end, special-state, and title rules are covered. Neo gradient/drop-shadow and dependency-marker rules remain deferred without emitted support. |
| Timeline | `timeline/styles.js` | Covered for current visible DOM, with disabled-rule caveat | Section fill/text/line colors, root colors, line wrappers, and event brightness rules are emitted for current Timeline DOM. Disabled node/text colors honor `tertiaryColor` and `clusterBorder`, but ordinary compact smoke sources emit no `class="disabled"` DOM, so disabled colors are provider coverage rather than public visible-signal coverage unless a fixture actually emits disabled nodes. Redux/neo-only rules stay deferred when support attributes/defs are absent. |
| Treemap | `treemap/styles.ts` | Covered | `treemap.*` options and title/text theme fallbacks drive section, leaf, label, value, and title CSS. |
| XYChart | none | Inline, render-path covered | Mermaid 11.15 has no dedicated provider. Visible theme behavior comes from `xyChart` theme config and inline renderer attributes; the custom-theme render-path smoke now covers background, title, axes, ticks, labels, and plot palette. |
| Error | none | Shared/error renderer | Not maintained as a full upstream SVG baseline family. No diagram-specific style provider exists in 11.15. |
| ZenUML | external plugin compatibility | Boundary | Local support is a headless Sequence compatibility subset, not Mermaid browser-plugin CSS parity. |

Unsupported Mermaid 11.15 families with style providers remain outside HPD-080 until admitted by
`docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`: `treeView`, `ishikawa`,
`eventmodeling`, `venn`, and `wardley`.

## Public Renderability Smoke

The public `HeadlessRenderer` dark-theme smoke now covers the supported families where a compact
source-backed theme/config signal is available: Architecture, Block, C4, Class, ER, Flowchart,
Gantt, GitGraph, Journey, Kanban, Mindmap, Packet, Pie, QuadrantChart, Radar, Requirement, Sankey,
Sequence, State, Timeline, Treemap, and XYChart.

The `resvg_safe` fixture smoke also has a separate boundary renderability gate for `info`,
`error`, and `zenuml`. That gate proves XML/raster-safe output for public boundary entrypoints,
including lenient suppressed-error rendering, but it does not promote those entries into the
implemented-family style-provider matrix.

The gate is intentionally semantic. It checks that output is SVG, geometry does not leak `NaN`,
unexpected `undefined` tokens are absent, representative labels remain visible in the output, and
diagram-owned theme colors or inline theme settings survive through the public API. It does not
attempt screenshot parity, font fallback parity, exact color-compositing parity, or counting CSS
rules as visible when the current renderer emits no matching elements.

C4 is covered through visible C4 config colors rather than broad generic `themeVariables`. This
matches the current Mermaid 11.15 boundary: the C4 stylesheet provider is narrow, while most visible
C4 palette behavior is C4 config or per-element style.

Known upstream placeholder class shapes are narrowly allowed:

- Kanban/shared cluster helpers can emit `class="cluster undefined ..."` and
  `class="node undefined"`.
- Timeline fixtures in the pinned Mermaid 11.15 baseline can emit
  `class="node-bkg node-undefined"`.

## Zed Feedback Boundary

Zed PR `zed-industries/zed#57967` is useful integration evidence but should not be copied as
default merman styling behavior.

- Zed's color cleanup keeps Zed's existing markdown preview appearance by replacing backgrounds and
  injecting editor-specific edge-label/tag-label colors. That is host policy.
- The fallback cleanup improvement is generic: a host may want to remove fallback overlays only when
  they duplicate native SVG text. merman now exposes that as
  `DropNativeDuplicateFallbacksPostprocessor`.
- Fallback markers are part of the public integration contract:
  `data-merman-foreignobject="fallback"` on fallback groups and
  `merman-foreignobject-fallback-text` on fallback text.

## Common Host Theme Needs

| Need | Current status | Notes |
| --- | --- | --- |
| Select an official Mermaid theme | Supported | Rust uses `HeadlessRenderer::with_site_config(...)`; binding consumers use top-level `options_json.site_config`; public metadata exposes all Mermaid 11.15 theme names. |
| Override Mermaid `themeVariables` | Supported | Rust can pass site config; shared binding options now expose top-level `site_config`; ordinary Mermaid init directives also work. Classic themes derive source-backed defaults, and `neo/redux*` extended themes now recompute visible derived keys such as edge-label, line, Architecture edge, Requirement relation/edge-label/neo border, Pie palette, Quadrant fill, actor/state background, GitGraph palette/inverse, and tag label colors from user base overrides while preserving explicit derived-key overrides. |
| Apply diagram-owned custom CSS | Supported | Mermaid `themeCSS` is emitted as scoped SVG CSS through the parity renderer, including when it comes from binding `site_config`. |
| Apply host-owned palette CSS | Supported | Rust consumers can append `ScopedCssPostprocessor` and optional `CssOverridePostprocessor`. Binding consumers can pass `svg.scoped_css` plus optional `svg.css_override_policy`; `resvg-safe` binding pipelines sanitize the injected CSS after insertion. |
| Rewrite arbitrary element attributes or inline styles | Host boundary | Rust consumers can write a custom `SvgPostprocessor`. Shared bindings intentionally do not expose a generic XML rewrite DSL; product-specific palette cleanup such as Zed's accent/tag/edge-label rules should remain host code unless a common, product-neutral contract emerges. |
| Export through resvg/usvg | Supported | `SvgPipeline::resvg_safe()` and binding `svg.pipeline="resvg-safe"` handle fallback insertion, `foreignObject` stripping, and common CSS/attribute hazards. |
| Remove duplicate fallback labels | Supported and opt-in | Rust uses `DropNativeDuplicateFallbacksPostprocessor`; bindings use `svg.drop_native_duplicate_fallbacks=true`. The pass is exact-text based: it preserves fallback-only text, but hosts with intentionally repeated labels should treat it as an optional cleanup, not a semantic de-duplication oracle. |
| Replace white SVG backgrounds with host background | Supported and opt-in | Rust uses `RootBackgroundPostprocessor`; bindings use `svg.root_background_color`. This changes only the root inline canvas color and does not rewrite Mermaid-owned node/edge/label palettes. |
| Match browser font fallback/raster output exactly | Boundary | merman should expose deterministic, headless measurements honestly rather than pretending browser font fallback is exact. |

## Negative Gates

Do not claim theme parity by adding inert CSS. A rule is useful only if the current renderer emits
the elements, attributes, defs, or filters that make the rule visible.

Do not globally strip or rewrite root `background-color: white;` from default emitted SVGs. Stored
upstream baselines include the capture-injected background and local parity output preserves that
shape. Hosts that need a different canvas color must opt in through `RootBackgroundPostprocessor`
or `svg.root_background_color`.

Do not make browser font metrics look exact by hardcoding fixture-specific widths. Continue using
the measurement seams from HPD-040 and classify residuals honestly.

Do not preserve invalid SVG tokens for byte parity when they break supported headless rendering.
The current QuadrantChart default point color is the narrow precedent: upstream's `hsl(...NaN%)`
comes from a missing khroma amount argument, so local SVG output uses a valid default and xtask
normalizes only that known default point-color slot in parity modes.

Do not keep useless invalid inline style attributes only because upstream fixtures contain them.
ER relationship paths and Mindmap edge paths now omit upstream's `style="undefined;;;undefined"`
artifact; their visible behavior remains class-driven.

Do not infer parser parity from a Mermaid DB helper in isolation. Treemap `TreeMapDB.addClass(...)`
has tolerant style splitting, but pinned Mermaid 11.15 renders `classDef ... color;` as an error
diagram; local parsing therefore rejects bare style tokens instead of treating DB-layer tolerance as
valid source syntax.

## Next Useful Work

1. Extend the dark-theme renderability smoke only when a newly supported diagram has a source-backed
   visible theme contract or a real consumer failure. Keep it semantic, not pixel-parity based.
2. Audit Info/Error only for actual user-visible failures, not for absent provider parity.
3. Add root-background smoke coverage only when a host reports a concrete raster/export failure;
   the output-policy seam is now explicit, so there is no reason to change defaults.
4. Extend `neo/redux*` override derivation only when fixture or consumer evidence points to a
   currently emitted surface not covered by the visible-key seam. Do not replace the generated
   default snapshots with broad fixture-keyed constants.
