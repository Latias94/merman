# Diagram Theme Coverage

This ledger tracks how the alpha.5 experimental `DiagramTheme` pipeline reaches supported SVG
families. It distinguishes direct typed consumption from the temporary Mermaid compatibility
bridge. A bridge mapping is useful rendering coverage, but it is not evidence that the typed
mechanism was applied by the family and must not be counted as portable output proof.

Default Mermaid-parity output remains unchanged. A compiled theme is opt-in, and the removed
`PresentationTheme`/`HostTheme` APIs are not compatibility aliases.

Field-level config merge and parser-entry evidence live in
`docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`. Source-style precedence lives in
`docs/alignment/MERMAID_THEME_STYLE_PRECEDENCE.md`. This ledger covers only the current
theme-to-family path and its visible residuals.

## Current Paths

| Diagram family | Current theme path | Evidence status | Residual / follow-up |
| --- | --- | --- | --- |
| State | Direct crate-private `ResolvedDiagramTheme` consumption in `state/style_plan.rs` and `state/label_artifact.rs`, including semantic targets, ordinal resolution, typography, and source-style provenance | Typed family adapter | The representative State output is one part of C6a; it does not prove the complete 18-cell Flowchart/State/Sequence by Standalone SVG/PNG ledger. |
| Flowchart and Swimlane | A family-local program is prepared with the render operation. Node and Edge paint, Edge dash arrays, selected Node geometry, NodeLabel/EdgeLabel font stack and size, and Node ordinal palettes have narrow direct consumers; remaining Mermaid/CSS surfaces still receive a family-scoped compatibility overlay. | Partial typed adapter plus legacy compatibility | HTML/Markdown edge labels, base typography, remaining geometry and surfaces, canvas/effects, and the representative SVG/PNG ledger remain open before C6a can close. |
| Sequence | Static unqualified Actor, Note, and Activation fill/stroke (solid/transparent) are consumed directly by the Sequence CSS/shape writers and reconciled from terminal emission receipts. Signal, loop, message, typography, label, variant, ordinal, and non-scalar paint routes remain compatibility-only or unsupported. | Partial typed adapter plus legacy compatibility | Actor/Note/Activation cutover routes have private SVG/PNG authorization witnesses and strict portability coverage; they do not count as C6 cells, and the representative Sequence matrix remains open. |
| Class and Block | Node-family semantic targets are projected through `LegacyFamilyThemeBridge`; renderers continue to consume final Mermaid variables and CSS | Legacy compatibility | Source/property provenance and family-owned typed emission are not yet complete. |
| Mindmap, Tree View, and GitGraph | Node/palette targets are projected through the family-local compatibility bridge, then consumed through final resolved Mermaid variables | Legacy compatibility | Palette projection is not direct ordinal-palette evidence at the SVG consumer. |
| Gantt and Kanban | Task, status, text, line, and palette targets are projected through the task-family bridge | Legacy compatibility | Family-local fixed colors and status details still require typed classification. |
| Requirement and ER | Dedicated bridge mappings produce the Mermaid variables consumed by the current renderers | Legacy compatibility | Marker, row, status/risk, and source-style channels still need direct typed evidence. |
| Pie, XY Chart, Quadrant Chart, and Radar | Chart and ordinal targets are projected through dedicated bridge mappings | Legacy compatibility | A visible palette in SVG is not proof that the typed ordinal mechanism survived every layout/output stage. |
| Timeline and Journey | Dedicated bridge mappings project text, line, activity, actor, and series values | Legacy compatibility | Some visible attributes retain family-local fallbacks. |
| Architecture, C4, Packet, Treemap, Ishikawa, EventModeling, Venn, and Sankey | The bridge supplies bounded text plus frozen family-specific compatibility values where available | Legacy compatibility | These families are outside the current C6 representative matrix and retain varying amounts of raw Mermaid-token consumption. |
| Info, Error, ZenUML, Cynefin, Wardley, and Railroad | Text-only or no-op compatibility contribution, depending on the family surface | Minimal compatibility | No positive typed visual-theme claim is made for these families. |

The compatibility bridge is deliberately family-local and runs only after detection. It never
projects one family's semantic targets into unrelated renderers. Its contribution IDs and
residuals remain observable so strict portability can reject legacy compatibility instead of
silently treating it as typed application.

## Portability Boundary

Theme compilation validates one bounded recipe, freezes its fingerprint, and reports required
capabilities and text-layout capabilities. Later stages own different facts:

| Stage | Evidence owned by the stage |
| --- | --- |
| Compile | Valid typed recipe, resource bounds, and declared/inferred requirements |
| Family/document | Actual family applicability, property winners, compatibility/source residuals, root paint/effects, and prepared text evidence |
| Export | Target-specific resource closure and SVG/PNG/JPEG/PDF admission |

These stages are monotonic: a later stage may preserve or weaken earlier evidence, but an
unevaluated family or output cannot be upgraded to portable. C6a is not yet proven because the
representative Flowchart/State/Sequence by Standalone SVG/PNG positive-output ledger is incomplete.

The current private cutover manifest contains 24 route-level authorization witnesses: six
Flowchart, six Swimlane, four Sequence Actor, four Sequence Note, and four Sequence Activation
routes, producing 32 artifact witnesses. These witnesses prove only that the named bridge
projections may be retired; they do not increase C6a's 18-cell count.
The active `acceptance/c6-v3.json` representative ledger remains C6a `2/18` (Standalone SVG and
PNG for Brutalist State).
Four historical cross-target observations exist for the same render group, but C6b equal-depth
certification is paused and has no active completion percentage. Browser SVG remains unstarted.

## Gates

- Every family cutover must replace a compatibility contribution with direct typed consumption and
  retain a residual for any mechanism that is still bridged or unsupported.
- Feature-bearing theme work must include focused family/document evidence under
  `crates/merman-render/tests/` or `crates/merman/tests/`.
- Tests must assert the DOM or exported surface that consumes the value, not only that a color or
  capability ID exists in the compiled recipe.
- Source-owned styles, raw `themeCSS`, custom postprocessors, host-dependent measurement, and
  browser-only behavior must retain their explicit evidence grades.
- Accepted residuals must be described instead of hidden by comparator normalization.
