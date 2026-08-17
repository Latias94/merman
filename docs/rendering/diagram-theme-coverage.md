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
| State | Direct crate-private `ResolvedDiagramTheme` consumption in `state/style_plan.rs` and `state/label_artifact.rs`, including semantic targets, ordinal resolution, typography, and source-style provenance | Typed family adapter | Brutalist, Spotless, and Cyberpunk State each have representative Standalone SVG and PNG C6 cells, accounting for six of the active ledger's 18 enforced cells. |
| Flowchart and Swimlane | A family-local program is prepared with the render operation. Node and Edge paint, Edge dash arrays, selected Node geometry, NodeLabel/EdgeLabel font stack and size, and Node ordinal palettes have narrow direct consumers; remaining Mermaid/CSS surfaces still receive a family-scoped compatibility overlay. | Partial typed adapter plus legacy compatibility | Brutalist, Spotless, and Cyberpunk Flowchart each have representative Standalone SVG and PNG C6 cells from one `RenderedDocument`. HTML/Markdown edge labels, base typography, remaining geometry and surfaces, and broader canvas/effect behavior remain open. |
| Sequence | Static unqualified Actor, Lifeline, Note, and Activation fill/stroke plus Message stroke (solid/transparent) are consumed directly by the Sequence CSS/shape writers and reconciled from terminal emission receipts. Native terminal text now carries prepared-text evidence into the final font seal. Message fill, signal/message text, loop, family typography, label, variant, ordinal, and non-scalar paint routes remain compatibility-only or unsupported. | Partial typed adapter plus legacy compatibility | Actor/Lifeline/Note/Activation/Message cutover routes have private SVG/PNG authorization witnesses. Brutalist, Spotless, and Cyberpunk Sequence each have representative Standalone SVG and PNG C6 cells from one `RenderedDocument`. |
| Class | Static relation `Edge` stroke width is resolved before layout and consumed by the terminal path writer; remaining node, label, paint, marker, and variant surfaces still use the family bridge or remain unsupported | Partial typed adapter plus legacy compatibility | Relation width has family-owned terminal evidence and paint-bounds coverage. Prepared typography and broader Class surfaces remain open. |
| Block | Node-family semantic targets are projected through `LegacyFamilyThemeBridge`; the renderer continues to consume final Mermaid variables and CSS | Legacy compatibility | Source/property provenance and family-owned typed emission are not yet complete. |
| Mindmap | `Node` ordinal palettes have a narrow family-local consumer for non-root branch-node fill at the Mindmap CSS/SVG writer. Root paint, edge and label colors, scalar Node paint, typography, variants, effects, and remaining surfaces continue through compatibility behavior or remain unsupported. | Partial typed adapter plus legacy compatibility | Branch section zero maps to palette ordinal one and the Mermaid eleven-slot section ring is preserved. Explicit owning `themeVariables.cScaleN` / `themeVariables.mainBkg` values retain precedence. Neo gradient surfaces and raw `themeCSS` remain residual. This migration is authorized by the compact C7b ordinal-palette ledger in KTD18; it is not part of the scalar C6 route manifest or the C6a cell count. |
| Tree View | Static unqualified `Edge` stroke width is resolved before layout and shared by connector geometry, paint bounds, the root viewBox, terminal line attributes, and family evidence; node paint, text, icons, and remaining edge facets still use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | `Clear` restores Mermaid's 1px line thickness. Explicit `treeView.lineThickness` configuration retains precedence; variants, ordinals, and mixed unsupported facets fail closed. |
| GitGraph | Node and palette targets are projected through the family-local compatibility bridge, then consumed through final resolved Mermaid variables | Legacy compatibility | Palette projection is not direct ordinal-palette evidence at the SVG consumer. |
| Gantt | Static unqualified `Task` radius is resolved before layout, written into every task bar, and reconciled from terminal SVG checkpoints; task paint, status, text, line, and palette targets still use the task-family bridge | Partial typed adapter plus legacy compatibility | `Clear` restores Mermaid's 3px task radius. Explicit variants, ordinals, mixed unsupported facets, fixed colors, and status details remain fail-closed or bridged. |
| Kanban | Static unqualified `Task` radius is resolved before layout, shared by card and priority-line geometry, and reconciled from terminal SVG checkpoints. `Task` ordinal palettes are applied directly to terminal card rectangles with Mermaid's light/dark adjustment; task stroke, status, text, and line targets still use the task-family bridge. | Partial typed adapter plus legacy compatibility | `Clear` restores Mermaid's 5px item radius. Sections keep their independent 5px baseline. Explicit `background` ownership outranks the typed task-card palette; `cScaleN` and `gitN` continue to own their separate section/root surfaces and no longer receive palette side effects from the retired bridge. The two former palette contribution IDs are retired under the compact KTD18 ordinal ledger; variants, ordinal rules, mixed unsupported facets, fixed colors, and status details remain fail-closed or bridged. |
| Requirement | Dedicated bridge mappings produce the Mermaid variables consumed by the current renderer | Legacy compatibility | Marker, row, status/risk, and source-style channels still need direct typed evidence. |
| ER | Static unqualified/default `Entity` solid or transparent fill and stroke are resolved by a family-local plan and consumed by the terminal entity-shell writer for both plain and attribute-table entities. Relation, text/title, table-row, and remaining facets still use compatibility behavior or remain unsupported. | Partial typed adapter plus legacy compatibility | Source `classDef`/`style` winners and the actual Mermaid `themeVariables.mainBkg` / `themeVariables.nodeBorder` owners retain precedence. `Clear`, gradients/patterns, ordinal Entity rules, markers, and broader ER styling remain fail-closed or bridged. This strengthens the C7a entity/card probe; it is not C6 coverage or broad ER theme support. |
| Pie | `PieSlice` ordinal palettes are resolved before layout and shared by slice paths, legend swatches, and terminal family evidence; remaining chart text, stroke, and non-ordinal routes still use compatibility behavior | Partial typed adapter plus legacy compatibility | Source/site `themeVariables.pieN` values retain precedence over typed palette slots. Broader chart styling remains open. |
| XY Chart | `ChartSeries` ordinal palettes are resolved once and shared by bar fill/stroke, line stroke, line point-label fill, layout data, terminal attributes, and family evidence. Axis, title, general text, scalar series paint, and remaining chart facets continue through compatibility behavior or remain unsupported. | Partial typed adapter plus legacy compatibility | Explicit source/site ownership of the complete `themeVariables.xyChart.plotColorPalette` token outranks the typed palette. The retired bridge's unrelated `accentColor` side effect is intentionally not reproduced. Solid/transparent values and palette cycling have strict final-SVG evidence under the KTD18 ordinal ledger. |
| Radar | Chart and ordinal targets are projected through dedicated bridge mappings | Legacy compatibility | A visible palette in SVG is not proof that the typed ordinal mechanism survived every layout/output stage. |
| Quadrant Chart | Static unqualified `ChartSeries` radius is resolved per point with inline, class, and explicit config precedence, then shared by layout, terminal circle emission, and family evidence; chart paint, axes, labels, palettes, and remaining geometry still use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | `Clear` restores Mermaid's configured point-radius baseline. Qualified selectors, unsupported ordinal winners, and mixed facets remain fail-closed. |
| Timeline | Static unqualified `TimelineEvent` opacity is emitted on each terminal event wrapper and reconciled only after the complete SVG root is sealed; event paint, text/title, line, and palette targets still use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | `Clear` restores the absent SVG opacity attribute. Variants, ordinals, per-channel opacity, and mixed unsupported facets fail closed. |
| Journey | Static unqualified `JourneyTask` radius is written to every terminal task rectangle while section geometry keeps its independent Mermaid baseline; text, line, activity, actor, paint, and series values still use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | `Clear` restores the 3px task radius. Qualified selectors, ordinals, and mixed facets remain fail-closed. |
| Architecture | Static `Cluster` clear/solid/transparent fill and stroke are resolved once and emitted as group-local inline paint after source/site group-border precedence; remaining text, service, edge, geometry, and variant routes use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | This is a C7a spatial/container shape witness, not C6 coverage or broad Architecture theme support. |
| C4 | Static unqualified `Cluster` radius is written only to explicit solid or dashed boundaries and reconciled from terminal boundary receipts; ordinary C4 elements keep their independent geometry, while paint, text, relationships, and remaining boundary facets still use compatibility behavior or remain unsupported | Partial typed adapter plus legacy compatibility | `Clear` restores the 2.5px boundary radius. The implicit global boundary is not a terminal occurrence and therefore does not satisfy the mechanism. |
| Packet, Treemap, Ishikawa, EventModeling, Venn, and Sankey | The bridge supplies bounded text plus frozen family-specific compatibility values where available | Legacy compatibility | These families are outside the current C6 representative matrix and retain varying amounts of raw Mermaid-token consumption. |
| Info, Error, ZenUML, Cynefin, Wardley, and Railroad | Text-only or no-op compatibility contribution, depending on the family surface | Minimal compatibility | No positive typed visual-theme claim is made for these families. |

Seventeen of the 33 concrete families now have at least one family-owned direct typed surface:
State, Flowchart, Swimlane, Sequence, Class, Gantt, Kanban, Pie, Architecture, ER, Mindmap,
Timeline, Tree View, Journey, Quadrant Chart, XY Chart, and C4. State remains the only family with no
`LegacyCompatibility` route. The other sixteen direct families are partial; 16/33 families have no family-local direct typed surface,
and 32/33 may still enter the compatibility bridge.

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
unevaluated family or output cannot be upgraded to portable. All 18 representative
Flowchart/State/Sequence by Standalone SVG/PNG cells execute successfully and the private harness
now seals that exact ledger with `C6aEligibilityReceipt`. Remaining C5 and C7a pre-freeze witnesses
still block the public authoring contract.

The current private cutover manifest contains 34 route-level authorization witnesses producing 46
artifact witnesses. These witnesses prove only that the named bridge projections may be retired;
they do not increase C6a's 18-cell count.
The active `acceptance/c6-v4.json` representative ledger preserves `c6-v3.json` as an immutable
predecessor and executes `18/18` cells across nine render groups: Brutalist, Spotless, and Cyberpunk
by Flowchart, State, and Sequence. Each versioned proof recipe projects Standalone SVG and PNG from
one completed `RenderedDocument`; no cells remain deferred. The eligibility issuer accepts only the
exact manifest lineage, nine paired group identities, portable target-owned receipts with embedded
fonts and no residuals, and the declared critical-mechanism union. Route-cutover receipts and
JPEG/PDF smoke results are outside this type boundary and cannot increase C6a eligibility. C7a
remains blocked by the separate C5 and pre-freeze authoring/consumer gates.
Four historical cross-target observations exist for the Brutalist State render group, but C6b
equal-depth certification is paused and has no active completion percentage. Browser SVG remains
unstarted.

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
