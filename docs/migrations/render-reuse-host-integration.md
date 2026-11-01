# Prepared rendering and host integration migration

This guide describes the current development branch, after published Merman `0.9.3`.
It does not change the contracts of installed `0.9.3` artifacts. Build Rust crates and generated
wrappers from the same revision; Web source builds require transport API `8`.
See the [release index](../release/README.md) for published-version migrations.

## Reuse parsed source in Rust

Use `Renderer::prepare_source(PrepareSourceRequest::new(source, control))` to create a
`PreparedSource`, then `Renderer::render_prepared(&prepared, target, fresh_control)` for each
output. The [redraw example](../../crates/merman/examples/redraw_prepared.rs) contains the complete
request wiring. `PresentationArtifact::into_prepared_source()` transfers an already parsed full
presentation artifact into the same reusable owner and releases its original operation control.

Canonical semantic inspection now uses `Renderer::parse_semantic()` or
`RenderRequest::semantic()`. Its `SemanticArtifact` has canonical metadata and a
`SemanticDiagramModel`, but no compatibility JSON or promotion to graphical output. Semantic
and ordinary ASCII requests do not evaluate visual theme programs. An admitted invalid visual
color can therefore succeed there while full presentation retains its original theme error.

Replace the old format-neutral `prepare_semantic()` API with `prepare_presentation()` wherever
the caller needs later graphical output or a reusable full source. Use
`RenderRequest::presentation()` / `RenderOutput::Presentation` for an explicit full artifact.
Select `compat-json` when calling `PresentationArtifact::compatibility_json()`; it implies
`presentation`. `svg` selects presentation, not the facade's compatibility-JSON API.

Configure canonical requests with `Renderer::with_semantic_parser(SemanticParser::new()
.with_site_config(config))`. Full custom detectors and adapters remain explicitly full-only
through `with_presentation_engine(MermaidEngine::new()...)`; they do not silently replace the
canonical parser. `with_runtime_policy()` sets both operation routes. The old `Engine` and
`ParseMetadata` aliases are removed; their full counterparts are `MermaidEngine` and
`PresentationMetadata`.

Without an explicitly installed full engine, presentation defaults inherit the canonical
parser's host configuration and runtime. Replacing that parser also replaces those inherited
defaults, even after an earlier full operation. An explicitly installed full engine retains its
own configuration and adapters; `with_runtime_policy()` deliberately updates both routes.

The remaining Rust source migrations are:

| Former use | Current use |
| --- | --- |
| Full site configuration as `MermaidConfig` | `SourcePresentationConfig::from_config(config)` for `MermaidEngine`; semantic parsers still accept `MermaidConfig` |
| `merman_core::geom` | `merman_render::geom`; geometry belongs to rendering |
| `ErrorDiagramRenderModel` | Canonical `merman_model::families::error::ErrorDiagramModel`, shared by semantic and full parsing |
| `SequenceDiagramRenderModel` | Canonical `merman_model::families::sequence::SequenceDiagramModel`, shared by semantic and full parsing |
| Moving Journey, Timeline or Packet fields out of a full carrier | Use the canonical payload directly; the redundant full wrappers have been removed |
| ASCII entrypoints taking `RenderSemanticModel` | `render_parsed` / `render_parsed_report` on a `ParsedSemanticDiagram`, or the explicit borrowed `AsciiDiagram` and `AsciiSourceContext` entrypoints |
| Custom full analyzer construction | Enable analysis `compat-json` and use `Analyzer::with_mermaid_engine`; ordinary analyzers remain canonical |

`editor-facts` does not enable presentation. Default analysis/editor captures retain syntax,
recovery, warnings and source-config evidence but no longer validate visual theme programs.
Request full presentation when the host needs preview/theme validation. Terminal and diagnostic owners select `merman-model/terminal-text`, independently of diagram selection.

The `complete-svg` and `complete-svg-elk` convenience features remain available on `merman`
and `merman-rustdoc`. They now select `svg,layout-cytoscape` and
`svg,layout-cytoscape,layout-elk`, respectively, without `math`. This is a behavior change from
their published math-enabled definitions: add `math` explicitly to preserve mathematical-label
rendering, for example `features = ["complete-svg-elk", "math"]`. Neither aggregate selects
diagram families; with defaults disabled, also select `all-diagrams` or the required `diagram-*`
features. Source defaults and exact artifact recipes continue to select direct capability leaves.

The snapshot freezes canonical semantics, effective configuration, parse options, the existing
compiled theme binding, source size/model complexity, and the actual parse-time runtime context.
Implicit Gantt dates and generated GitGraph IDs therefore remain fixed across redraws. Prepare
again when source, parse settings, site configuration or parse-time dates/IDs must change.
Exported semantic JSON cannot reconstruct a snapshot.

Each output reads its own layout options and text services and receives fresh cancellation,
deadline, input admission, work accounting and output runtime state. An output-owned Gantt today
marker can change without changing parsed task dates. Reuse does not cache a completed layout or
grant a source/model quota exemption. `render_prepared_with_resource_policy` supplies an explicit
input override; otherwise current renderer input policy takes precedence over target defaults.
Completed SVG, layout, geometry and document results own their public data and remain usable
after dropping the snapshot. Existing `RenderedDocument` exports continue to reuse a completed
SVG when no relayout is needed.

Low-level `merman-render::family::prepare` borrows `&merman_model::presentation::PresentationDiagram`. For source results, call `prepare(parsed.presentation(), ...)`, or transfer the complete input with `parsed.into_presentation()`. Keep that presentation owner alive for the lifetime of `FamilyRenderArtifact<'_>`; preparation owns its layout/session while borrowing canonical model and output context. See the [model/source migration](model-source-boundaries.md) for independent model construction and all import changes.
`layout_json_with_compatibility_json` was removed: request `artifact.layout_json()` so semantics
are projected from the exact artifact that owns the layout, under its active control.

## Use canonical family type names

Pure public render aliases have been removed. Use the canonical types from
`merman_model::families::<family>` in semantic and full consumers, including imports,
reexports, model construction, and explicit type annotations. The same canonical spelling
now describes the shared family data on both paths; selecting `presentation` does not add
an alternate name for that data. These types remain available with their diagram family
without requiring `presentation`.

This is a Rust source migration. It does not rename parse/layout/render functions, change
compatibility JSON fields, or change the full parser's capability requirements. Upgrade
coupled crates together. The following table includes the older Cynefin and Railroad aliases
so that consumers can retire every pure family render spelling in one pass. Error and
Sequence migrations are listed in the preceding source-migration table.

| Family module | Removed spelling -> canonical spelling |
| --- | --- |
| `architecture` | `ArchitectureDiagramRenderModel` -> `ArchitectureDiagramModel`<br>`ArchitectureRenderNodeType` -> `ArchitectureNodeKind`<br>`ArchitectureRenderNode` -> `ArchitectureNodeModel`<br>`ArchitectureRenderGroup` -> `ArchitectureGroupModel`<br>`ArchitectureRenderEdge` -> `ArchitectureEdgeModel`<br>`ArchitectureRenderLayoutHint` -> `ArchitectureLayoutHintModel` |
| `block` | `BlockNodeRenderModel` -> `BlockNode`<br>`BlockEdgeRenderModel` -> `BlockEdge`<br>`BlockClassDefRenderModel` -> `BlockClassDef` |
| `c4` | `C4DiagramRenderModel` -> `C4DiagramModel`<br>`C4ShapeRenderModel` -> `C4Shape`<br>`C4BoundaryRenderModel` -> `C4Boundary`<br>`C4RelRenderModel` -> `C4Relation` |
| `cynefin` | `CynefinDiagramRenderModel` -> `CynefinDiagramModel` |
| `er` | `ErDiagramRenderModel` -> `ErDiagramModel`<br>`ErAttributeRenderModel` -> `ErAttribute`<br>`ErRelSpecRenderModel` -> `ErRelSpec`<br>`ErRelationshipRenderModel` -> `ErRelationship`<br>`ErClassDefRenderModel` -> `ErClassDef`<br>`ErSubgraphRenderModel` -> `ErSubgraph`<br>`ErEntityRenderModel` -> `ErEntity` |
| `event_modeling` | `EventModelingDiagramRenderModel` -> `EventModelingDiagramModel`<br>`EventModelingFrameRenderModel` -> `EventModelingFrame`<br>`EventModelingDataEntityRenderModel` -> `EventModelingDataEntity` |
| `gantt` | `GanttRenderTaskRaw` -> `GanttTaskRaw`<br>`GanttRenderTaskStart` -> `GanttTaskStart`<br>`GanttRenderTaskEnd` -> `GanttTaskEnd`<br>`GanttRenderClickEvent` -> `GanttClickEvent` |
| `git_graph` | `GitGraphRenderModel` -> `GitGraphDiagramModel`<br>`GitGraphBranchRenderModel` -> `GitGraphBranch`<br>`GitGraphCommitRenderModel` -> `GitGraphCommit` |
| `ishikawa` | `IshikawaDiagramRenderModel` -> `IshikawaDiagramModel`<br>`IshikawaNodeRenderModel` -> `IshikawaNode` |
| `journey` | `JourneyRenderTask` -> `JourneyTask` |
| `pie` | `PieRenderSection` -> `PieSection` |
| `quadrant_chart` | `QuadrantChartRenderModel` -> `QuadrantChartDiagramModel` |
| `radar` | `RadarRenderAxis` -> `RadarAxis`<br>`RadarRenderCurve` -> `RadarCurve`<br>`RadarRenderOptions` -> `RadarOptions` |
| `railroad` | `RailroadDiagramRenderModel` -> `RailroadDiagramModel` |
| `requirement` | `RequirementDiagramRenderModel` -> `RequirementDiagramModel`<br>`RequirementRenderNode` -> `RequirementNode`<br>`RequirementRenderElement` -> `RequirementElement`<br>`RequirementRenderRelationship` -> `RequirementRelationship`<br>`RequirementRenderClass` -> `RequirementClass` |
| `sankey` | `SankeyRenderNode` -> `SankeyNode`<br>`SankeyRenderLink` -> `SankeyLink`<br>`SankeyRenderGraph` -> `SankeyGraph`<br>`SankeyDiagramRenderModel` -> `SankeyDiagramModel` |
| `state` | `StateDiagramRenderLink` -> `StateLink`<br>`StateDiagramRenderLinks` -> `StateLinks`<br>`StateDiagramRenderNote` -> `StateNote`<br>`StateDiagramRenderState` -> `StateRecord`<br>`StateDiagramRenderRelation` -> `StateRelation`<br>`StateDiagramRenderStyleClass` -> `StateStyleClass` |
| `timeline` | `TimelineRenderTask` -> `TimelineTask` |
| `tree_view` | `TreeViewDiagramRenderModel` -> `TreeViewDiagramModel`<br>`TreeViewNodeRenderModel` -> `TreeViewNode` |
| `treemap` | `TreemapNodeRenderModel` -> `TreemapNode`<br>`TreemapClassDefRenderModel` -> `TreemapClassDef` |
| `usecase` | `UsecaseDiagramRenderModel` -> `UsecaseDiagramModel` |
| `venn` | `VennSubsetRenderModel` -> `VennSubsetModel`<br>`VennTextNodeRenderModel` -> `VennTextNodeModel`<br>`VennStyleEntryRenderModel` -> `VennStyleEntryModel`<br>`VennDiagramRenderModel` -> `VennDiagramModel` |
| `wardley` | `WardleyDiagramRenderModel` -> `WardleyDiagramModel`<br>`WardleyPointRenderModel` -> `WardleyPoint`<br>`WardleyNodeRenderModel` -> `WardleyNode`<br>`WardleyLinkRenderModel` -> `WardleyLink`<br>`WardleyTrendRenderModel` -> `WardleyTrend`<br>`WardleyPipelineRenderModel` -> `WardleyPipeline`<br>`WardleyAnnotationRenderModel` -> `WardleyAnnotation`<br>`WardleyNoteRenderModel` -> `WardleyNote`<br>`WardleyAcceleratorRenderModel` -> `WardleyAccelerator`<br>`WardleyDeacceleratorRenderModel` -> `WardleyDeaccelerator`<br>`WardleyAxesRenderModel` -> `WardleyAxes`<br>`WardleySizeRenderModel` -> `WardleySize` |
| `xy_chart` | `XyChartAxisRenderModel` -> `XyChartAxisModel`<br>`XyChartPlotRenderModel` -> `XyChartPlotModel` |
| `zenuml` | `ZenumlDiagramRenderModel` -> `ZenumlDiagramModel` |

State's shared `StateLink`, `StateLinks`, `StateNote`, `StateRecord`, `StateRelation`, and
`StateStyleClass` are exported directly from `merman_model::families::state`. Import them there instead
of using the removed `StateDiagramRender*` reexports. The full State model still owns its
render nodes, render edges, and document graph.

Actual presentation carriers keep their names and gates. In particular:

- `BlockDiagramRenderModel` / `BlockVisuals` and `TreemapDiagramRenderModel` /
  `TreemapVisuals` retain rendering state alongside canonical records.
- `AgentflowDiagramRenderModel` / `AgentflowVisuals` retain parser-assigned DOM ordinals.
- `GanttDiagramRenderModel` and `GanttRenderTask` retain display mode and rendered task ends;
  their raw/start/end/click records use the canonical names in the table.
- `StateDiagramRenderModel`, `StateDiagramRenderNode`, `StateDiagramRenderEdge`, and
  `XyChartDiagramRenderModel` retain their presentation data. Other actual render carriers,
  including Class, Flowchart, Mindmap, and Kanban records, are likewise unchanged.

Journey, Timeline, Packet, Pie, Radar, and Info instead use their canonical family models
directly in `RenderSemanticModel`. Remove the redundant `*DiagramRenderModel` wrapper and
its `semantic` field when migrating these families. Preserve output-specific context through
the complete `PresentationDiagram` handoff rather than reconstructing it from the raw model.
See the [model/source migration](model-source-boundaries.md#pass-presentation-data-to-graphical-consumers).

## Migrate edge geometry budgets

`edge-geometry-json` retains payload schema `1`, source occurrence order, duplicate edge IDs,
the emitted `data-points` coordinates and painted label centers. Flowchart, Swimlane and Agentflow
share their actual family route/label decisions with SVG. The query no longer emits or discards an
SVG; `layout-json` still exposes the earlier layout coordinates.

Replace geometry-specific `max_svg_bytes` overrides with `max_geometry_json_bytes`. The binding
operation accepts source/model limits, `max_layout_work_units`, `max_prepared_text_retained_bytes`
and the new encoded-output ceiling. Explicit SVG, raster or other unrelated limits on this
operation are rejected. SVG and native export operations likewise reject an explicit
`max_geometry_json_bytes` override. Constructor options can still describe the union of limits
used by their compiled operations; each output enforces its applicable dimensions.

Rust's typed geometry result uses construction/work limits. Encode it with
`EdgePaintGeometryOutput::write_json_controlled` to enforce exact JSON bytes and cancellation;
raw serde serialization does not supply that operation contract. Binding and CLI encoded geometry
use the maintained writer. Geometry resource failures report phase `geometry_output` for the
encoded ceiling. Ordinary SVG retains its SVG budgets.

Successful built-in families without geometry return empty edges after input/option admission,
without starting unused layout, font or SVG services. A custom JSON model remains non-renderable.
`RequirePortable` reports that portability is unavailable for this target rather than certifying
a hidden SVG. These are intentional changes to incidental errors from the former SVG path.
Native ABI `3` operation ordinals and the geometry payload shape remain unchanged. See
[resource options](../bindings/OPTIONS_JSON.md#resource-options).

## Reuse and publish in a browser

Rendering-capable full/render packages expose `prepareSource()` and `mountDiagram()`; analysis,
editor and ASCII packages retain their existing surfaces. A prepared wrapper is bound to the WASM
runtime that created it. Its output methods accept output-only options; changing parse/config/time
settings or the existing theme binding requires another preparation. Call `dispose()` when done.
Disposal is idempotent, calls after disposal fail, and disposal inside a host measurement callback
defers freeing active Rust data until that call returns.

`mountDiagram` owns candidate publication, snapshot retention, document-local resize/font resources
and generated SVG IDs. Failed updates retain the last successful diagram. `update(source)` retains
options; supplying options replaces them. `refresh()` handles external typography changes and
explicit adoption into another Document. Hidden, disconnected or zero-width targets defer work.
`destroy()` releases resources while leaving the last published DOM in place. The same source,
actual-root and navigation admission used by one-shot DOM helpers runs before publication.
See the [Web lifecycle guide](../../platforms/web/README.md#maintain-a-mounted-diagram) for code.

Measurement remains deterministic by default. Actual browser glyph widths can exceed those
headless estimates even when a font is installed; fit-width placement does not remove that
measurement boundary. Select `textMeasurement: "browser"` on a mount, or supply a host measurer,
when layout must use the final browser font metrics. Reuse the prepared source when fonts change;
do not compensate with arbitrary padding or smaller text.

## Request width adaptation explicitly

`LayoutAdaptation::FitWidth` / `layout.adaptation: "fit-width"` currently applies to Kanban.
It packs complete columns in source order, preserving labels, font sizes, tasks, IDs and links.
The default `None` / `"none"` retains Mermaid placement. Other families reject explicit
fit-width. A column wider than the container retains its natural width and reports overflow.

Read `layout_adaptation` on result metadata, or `meta.layout_adaptation` in layout JSON, for
`mode`, `requested_width`, `actual_width` and `overflowed`. A mount supplies its measured content
width for fit-width requests. This is family placement, independent of SVG viewport scaling.
See [headless layout](../rendering/HEADLESS_LAYOUT.md#explicit-kanban-width-adaptation).

## Validation boundaries

Internal strict and paired lenient parse paths compare complete managed compatibility projections
and result categories. Generated layout checks use bounded, supported Dagre/Swimlane subsets.
The independent [selected Mermaid differential](../security/FUZZING.md#selected-mermaid-parser-differential)
observes real Flowchart/Sequence parser and database behavior, retaining exact replay inputs and
separating tooling failures from grammar rejection. These checks supplement SVG/browser evidence;
they do not establish complete grammar coverage or a performance improvement.

Existing theme behavior remains regression coverage. This migration adds no dynamic recoloring,
CSS-variable theme inheritance or new theme-authoring contract.
