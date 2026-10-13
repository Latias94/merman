# Diagram Theme Coverage

This ledger tracks how supported SVG diagram families consume Mermaid theme variables and presentation theme roles. It is intentionally semantic: tests should assert visible SVG theme signals or document why a family cannot be covered through generic roles yet.

Default parity output remains unchanged. Host theme behavior is opt-in.

Field-level config merge and parser-entry evidence live in `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`.
This ledger only records the SVG/theme consumption layer and the visible residuals for admitted
families.

| Diagram family | Current theme path | Presentation theme status | Residual / follow-up |
| --- | --- | --- | --- |
| Flowchart | `PresentationTheme::node_diagram()` via `crates/merman-render/src/svg/parity/flowchart/css.rs` | Covered for node, text, border, line, cluster, edge label roles. | KaTeX and some special-shape details still have local hard-coded defaults. |
| Block | Reuses node diagram theme in `crates/merman-render/src/svg/parity/block.rs` | Covered for node, edge, cluster roles, and resvg-safe fallback label text inheritance. | Cluster background follows Mermaid's fade semantics (`rgba(..., 0.5)`) rather than emitting the raw role color. |
| Class | `PresentationTheme::class_diagram()` in `crates/merman-render/src/svg/parity/class/css.rs` | Covered for class text, node, border, cluster, note roles. | Note, gradient, and shadow roles are still partly scattered. |
| Sequence | `PresentationTheme::sequence_diagram()` in `crates/merman-render/src/svg/parity/sequence/css.rs` | Covered for actor, signal, note, label box, activation roles. | Some visible SVG attributes rely on CSS override rather than initialized themed attrs. |
| State | `PresentationTheme::state_diagram()` in `crates/merman-render/src/svg/parity/state/style.rs` | Covered for transition, state, label, note, special-state roles. | Marker fill remains mostly CSS-driven. |
| ER | Shared ER CSS and resolved `theme_token` reads | Covered for entity box, relationship line, text, and border signals. | Needs an `er_diagram()` view for marker, row fill, and shadow roles. |
| Requirement | Shared requirement CSS and visible node attrs in `crates/merman-render/src/svg/parity/requirement.rs` | Covered for requirement node surface, border, text, and relationship line roles. | Requirement-specific status/risk decorations may need additional semantic roles later. |
| Architecture | Architecture CSS reads `archEdge*` and `archGroup*` variables | Covered for edge and group border roles. | Built-in icon foreground/background is not fully themeable. |
| C4 | C4 renderer reads `c4.*` config defaults | Covered for element and boundary surfaces, borders, text, relationship text/lines, and family typography. | Source-local C4 styles and explicit config override presentation defaults. |
| ZenUML | Opt-in nested `themeVariables.zenuml` values | Covered for frame, participant, group, fragment, note, activation, message, and typography roles. | Plain Mermaid themes retain the plugin's original defaults; presentation supplies the nested bridge. |
| Mindmap | Final resolved `git*` and `cScale*` tokens plus `mindmap.labelBackgroundColor` | Covered through series palette and label background bridges, including resvg-safe icon labels. | Family rendering projects the core-resolved scale without deriving a second palette. |
| Kanban | Local `git*` and `cScale*` palette reads | Covered through series palette bridge and common roles. | Disabled states and root background remain local defaults. |
| Timeline | `PresentationTheme::timeline()` | Covered through `cScale*`, `git*`, text, and line variables. | Some visible line attrs still have local black defaults. |
| GitGraph | Final resolved `git*` and `gitBranchLabel*` token reads | Covered through series palette bridge. | Merge/cherry-pick inner marks keep fixed colors. |
| XY Chart | `PresentationTheme::xychart()` and nested `xyChart` variables | Covered for series, canvas, title, legend, axis text/lines, and root CSS. Layout measures the configured font family. | Explicit XY Chart config continues to control data labels and family typography. |
| Quadrant Chart | `PresentationTheme::quadrantchart()` during layout | Covered through quadrant fill/text/border variables. | SVG stage is layout-driven and does not carry a separate theme view. |
| Pie | Pie CSS and final resolved `pie1..pie12` theme variables | Covered through series palette bridge and pie text/border roles. | Layout and legend consume one resolved scale; the renderer does not derive another palette. |
| Sankey | `sankey.*` config and default Tableau palette | Not generically covered by series palette. | Node colors are keyed by node id; use raw `sankey.nodeColors` or SVG postprocessing. |
| Radar | Local `SvgTheme` reads `radar.*` and `cScale*` | Covered through generated `radar.*`, series palette, and common roles. | Needs a `RadarTheme` view in `PresentationTheme` to reduce local reads. |
| Treemap | `treemap.*` config and `cScale*` labels | Covered through generated `treemap.*` and common roles. | Needs a `TreemapTheme` view to reduce local reads. |
| Venn | Local `venn*` and text variables | Covered through series palette bridge and common text roles. | Needs a `VennTheme` view for text and fill readability. |
| Gantt | Shared Gantt CSS variables plus `gantt.axisColor` | Covered for task, done, critical, section, text, and grid roles, including D3 axis `currentColor` inheritance. | Axis attributes retain the upstream DOM shape; scoped CSS supplies their color. |
| Wardley | Nested `themeVariables.wardley` values | Covered for canvas, grid, axes, components, anchor labels, stage dividers, links, overlays, annotations, and font family. | Explicit Wardley styles override the semantic bridge; geometry remains family-owned. |
| Usecase | Node theme variables and `usecase.actor*` / `usecase.usecase*` config | Covered for node roles and actor/usecase font family and size. | Source-local family typography overrides presentation defaults. |
| Journey | Local `fillType*` and `actor*` reads | Covered through series palette bridge and common roles. | Activity line and actor stroke still include fixed colors. |
| Packet | `packet.*` diagram config | Covered through generated packet text, block, and byte colors. | Packet base CSS does not yet use common theme roles directly. |
| Tree View | `PresentationTheme::tree_view()` | Covered through nested `themeVariables.treeView.labelColor` and `lineColor`. | No fallback from common `textColor` without semantic theme mapping. |
| Ishikawa | `PresentationTheme::ishikawa()` | Covered for line, fill, text, font roles. | Theme surface is intentionally narrow. |
| EventModeling | `PresentationTheme::eventmodeling()` | Covered through common and `em*` roles for lanes, UI, command, event, read model, and relations. | The theme compiler should grow more explicit Event Modeling role mapping if needed. |
| Info | Static informational SVG plus common CSS | Low coverage required. | Include in common smoke only. |
| Error | Internal diagnostic diagram | Not in current bundled theme coverage. | Consider diagnostic dark-theme readability separately. |

## Gates

- Every new supported SVG diagram family must be added to this ledger.
- Feature-bearing theme work should include a focused SVG test under `crates/merman/tests/` or `crates/merman-render/tests/`.
- A test should assert colors on the DOM surface that currently consumes the role, not only that a color string exists somewhere in the SVG.
- Accepted residuals must be described in the table instead of hidden by comparator normalization.
