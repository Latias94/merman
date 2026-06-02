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
| Custom Mermaid theme variables | Product requirement | Renderers pass `effective_config` into CSS or inline style generation. Non-color CSS tokens use string/number-aware paths such as `SvgTheme::css_value(...)` where needed. |
| Browser-free raster-safe output | Product requirement | `SvgPipeline::resvg_safe()` inserts SVG text fallbacks for `<foreignObject>` labels, strips unsupported foreignObject content, and sanitizes CSS/attributes for resvg. |
| Host palette replacement, such as Zed markdown preview colors | Host integration boundary | Hosts should pass Mermaid config and/or compose postprocessors. merman should not inject Zed-specific edge-label, tag-label, or background colors by default. |
| Native/fallback duplicate label cleanup | Optional host pipeline feature | `DropNativeDuplicateFallbacksPostprocessor` drops only fallback groups whose text duplicates native SVG `<text>`, preserving fallback-only labels. |
| Exact browser font fallback and glyph rasterization | Host/rendering boundary | merman can expose configured font families and deterministic measurement approximations, but it should not claim browser font fallback parity in a headless renderer. |
| Root background replacement | Open integration boundary | Many stored upstream SVG baselines currently include `background-color: white;`. Do not globally strip this until the upstream capture path and current source behavior are reconciled. Hosts can still postprocess backgrounds for their UI. |
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
| C4 | `c4/styles.js` | Covered, narrow | Upstream provider only emits `.person`. Most C4 visible colors are inline C4 config or per-shape values, not generic themeVariables. Do not promote unrelated theme keys into C4 shapes. |
| Class | `class/styles.js` | Covered | Current-output node shape, divider, cluster, class-label, edge-terminal, relation, and note colors are covered. Icon and neo-only rules remain deferred until local output emits the required support elements/attributes. |
| ER | `er/styles.ts` | Covered | Entity, label, relationship, marker, edge-label, error, and current neo stroke rules are covered. `data-color-id` and unsupported neo label-background selectors remain deferred. |
| Flowchart | `flowchart/styles.ts` | Covered for current HPD-080 gaps | `strokeWidth` now reaches node and edge-path CSS. Larger Flowchart CSS parity should stay fixture/source driven because many rules are tied to emitted shape attributes and renderer-specific DOM. |
| Gantt | `gantt/styles.js` | Covered | Section, grid, today marker, task state, outside label contrast, marker, and title theme variables are emitted. |
| GitGraph | `git/styles.js` | Covered | Branch label, commit, arrow, merge, reverse, highlight, and label color rules are emitted for classic/default branch theme variables. |
| Info | none | Inline/shared base | No diagram-specific Mermaid provider in 11.15. Shared base CSS is sufficient unless a visible fixture proves otherwise. |
| Journey | `user-journey/styles.js` | Covered | Task, section, actor, arrowhead, edge-label, and fillType theme variables are emitted. |
| Kanban | `kanban/styles.ts` | Covered | Section, ticket, icon, and label theme CSS fixes the dark-card/hidden-label defect class. |
| Mindmap | `mindmap/styles.ts` | Covered with deferred rules | Section/root/icon/span colors are covered. `data-look` gradient/drop-shadow rules are deferred until local output emits matching attributes/defs. |
| Packet | `packet/styles.ts` | Covered | `packet.*` style options drive byte, label, title, and block CSS. |
| Pie | `pie/pieStyles.ts` | Covered | Stroke, opacity, title, slice, legend, font family, and text colors read Mermaid 11.15 theme variables. |
| QuadrantChart | `quadrant-chart/quadrantDiagram.ts` uses `styles: () => ''` | Inline | Theme behavior is inline through quadrant chart config, classDef, and point styles. No CSS provider should be invented. |
| Radar | `radar/styles.ts` | Covered | Top-level `radar.*` overrides are resolved before `themeVariables.radar.*`, matching Mermaid's clean-and-merge behavior. |
| Requirement | `requirement/styles.js` | Covered | Requirement boxes, relationship lines, labels, edge-label backgrounds, node text, and divider colors are covered. `data-look`/`data-color-id` rules are deferred where local output lacks attributes. |
| Sankey | `sankey/styles.js` | Covered | Label, outlined-label background, node, and link style options are emitted. |
| Sequence | `sequence/styles.js` | Covered with deferred rules | Actor, lifeline, signal, label, loop/section, note, activation, marker/error, and rect-node theme variables are covered. Neo-only selectors remain deferred without matching local DOM. |
| State | `state/styles.js` | Covered with deferred rules | State node, cluster, transition, label, note, marker, start/end, special-state, and title rules are covered. Neo gradient/drop-shadow and dependency-marker rules remain deferred without emitted support. |
| Timeline | `timeline/styles.js` | Covered | Disabled node/text colors now honor `tertiaryColor` and `clusterBorder`. Redux/neo-only rules stay deferred when support attributes/defs are absent. |
| Treemap | `treemap/styles.ts` | Covered | `treemap.*` options and title/text theme fallbacks drive section, leaf, label, value, and title CSS. |
| XYChart | none | Inline, render-path covered | Mermaid 11.15 has no dedicated provider. Visible theme behavior comes from `xyChart` theme config and inline renderer attributes; the custom-theme render-path smoke now covers background, title, axes, ticks, labels, and plot palette. |
| Error | none | Shared/error renderer | Not maintained as a full upstream SVG baseline family. No diagram-specific style provider exists in 11.15. |
| ZenUML | external plugin compatibility | Boundary | Local support is a headless Sequence compatibility subset, not Mermaid browser-plugin CSS parity. |

Unsupported Mermaid 11.15 families with style providers remain outside HPD-080 until admitted by
`docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`: `treeView`, `ishikawa`,
`eventmodeling`, `venn`, and `wardley`.

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

## Negative Gates

Do not claim theme parity by adding inert CSS. A rule is useful only if the current renderer emits
the elements, attributes, defs, or filters that make the rule visible.

Do not globally strip root `background-color: white;` from emitted SVGs until the stored upstream
baselines, Mermaid 11.15 source path, and CLI capture behavior are reconciled. This may become a
host postprocessor or an explicit output policy, but it should not be a silent default change.

Do not make browser font metrics look exact by hardcoding fixture-specific widths. Continue using
the measurement seams from HPD-040 and classify residuals honestly.

## Next Useful Work

1. Add a small dark-theme visual smoke set across the implemented matrix, focused on unreadable
   labels and black blocks rather than pixel parity.
2. Audit Info/Error only for actual user-visible failures, not for absent provider parity.
3. Consider a documented host-theme postprocessor example for consumers that want Zed-like palette
   replacement.
4. Reconcile the root white-background question as a separate source/capture audit before changing
   default SVG output.
