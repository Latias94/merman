# Family data migration inventory

Status: M7 implementation ownership reconciled against the shared worktree on 2026-10-30; theme-acceptance source callers are migrated, with acceptance-cfg validation pending. See [Current implementation reconciliation](#current-implementation-reconciliation-2026-10-30) for the final owners and evidence limits. This is a source inventory, not an M8 execution or completion receipt.

Original planning status, retained for history: planning checklist, 2026-10-30. Source: `8888358b1873c89049c0e430615c91d406dac224`. Implementation was not complete. This inventory supports [Model, Source, and Consumer Boundaries](2026-10-30-model-and-source-boundaries.md); it is not a new machine input or a source-analysis gate. The original tables, extraction constraints, phase description and checklist below describe that baseline; the appended reconciliation supersedes their ownership status without rewriting the historical inventory.

The baseline has 34 `SemanticDiagramModel` payloads, including Error infrastructure. Swimlane shares the Flowchart payload while keeping its own logical identity and source admission. Railroad dialects likewise share data without collapsing their language identities. No new diagram or ASCII handler is introduced by moving a payload.

In the table, paths are relative to `crates/merman-core/src/`. Each destination is under `merman_model::families`. “Same” means full processing currently uses the canonical payload; “distinct” means real full presentation data must survive; “remove wrapper” means compatibility-only state is separated without deleting compatibility behavior.

## Complete canonical and full inventory

| Payload | Current data source | Destination | Full data / terminal action |
| --- | --- | --- | --- |
| Error | `diagrams/error_diagram.rs` | `error` | Same; core constructs suppressed errors from its own `Error`. |
| Flowchart | `diagrams/flowchart/model.rs` | `flowchart` | Same, plus required render-context facts in presentation data; terminal provenance serializer retained. Swimlane shares this type. |
| Sequence | `diagrams/sequence/model.rs` | `sequence` | Same; retain lifecycle/message semantics and pure borrowed serialization used by terminal fallback. |
| State | `diagrams/state/{model,ast,complexity,terminal}.rs` | `state` | Distinct full nodes/edges/model; move document ownership and both borrowed-view branches. |
| Timeline | `diagrams/timeline.rs` | `timeline` | Remove wrapper; terminal borrows canonical model; core/projection preserves empty-output evidence. |
| Cynefin | `diagrams/cynefin.rs` | `cynefin` | Same; no terminal adapter added. |
| Info | `diagrams/info.rs` | `info` | Remove wrapper; retain compatibility presence/empty-output evidence. |
| Journey | `diagrams/journey.rs` | `journey` | Remove wrapper; terminal borrows canonical model; preserve section occurrence and NaN witnesses. |
| Pie | `diagrams/pie.rs` | `pie` | Remove wrapper; preserve compatibility empty/model distinction. |
| Packet | `diagrams/packet.rs` | `packet` | Remove wrapper; rename canonical `PacketRenderBlock` to `PacketBlock`; retain validation and terminal behavior. |
| TreeView | `diagrams/tree_view.rs` | `tree_view` | Same; retain current child ownership and terminal behavior. Do not mix in a separate representation redesign. |
| QuadrantChart | `diagrams/quadrant_chart.rs` | `quadrant_chart` | Same; retain styles, points, quadrants and axes. |
| Radar | `diagrams/radar.rs` | `radar` | Remove wrapper; retain JSON numeric payloads and compatibility witnesses. |
| Sankey | `diagrams/sankey.rs` | `sankey` | Same; preserve JSON link values; CSV parsing/cleaning remains core. |
| Venn | `diagrams/venn.rs` | `venn` | Same; retain subset/text/style data. |
| Requirement | `diagrams/requirement.rs` | `requirement` | Same; retain requirement/element/relationship/class vocabulary. |
| Usecase | `diagrams/usecase.rs` and pure codecs under `diagrams/usecase/` | `usecase` | Same; preserve UTF-16/lossless JSON, property order, surrogate and non-finite-number behavior. |
| EventModeling | `diagrams/eventmodeling.rs` | `event_modeling` | Same; normalize the Rust module spelling only. |
| Wardley | `diagrams/wardley.rs` | `wardley` | Same; retain all strategy/flow/axis/node/link/annotation data. |
| Railroad | `diagrams/railroad.rs` | `railroad` | Same; public `RailroadAstNode` is model data and moves. Dialect lexers/parsers do not. |
| Zenuml | `diagrams/zenuml/model.rs` | `zenuml` | Same; retain iterative typed serializer and span data; remove the empty sanitizer method. |
| Er | `diagrams/er.rs` | `er` | Same; terminal directly borrows model; retain label selection and relationship vocabulary. |
| Architecture | `diagrams/architecture.rs` | `architecture` | Same; retain layout hints and direction data. |
| Gantt | `diagrams/gantt/{model,terminal}.rs` | `gantt` | Distinct full task/model and borrowed view; preserve canonical versus full endpoint serialization. |
| C4 | `diagrams/c4.rs` | `c4` | Same; retain JSON-bearing text/optional values and layout vocabulary. |
| Ishikawa | `diagrams/ishikawa.rs` | `ishikawa` | Same flat data; nested compatibility projection becomes an upper-owner free function. |
| Treemap | `diagrams/treemap.rs` | `treemap` | Distinct full visuals/model; retain flat nodes and `ManagedJson`; separate compatibility-empty state. |
| Block | `diagrams/block.rs` | `block` | Distinct full visuals/model; preserve flat IDs, warnings and real palette data; separate compatibility presence witnesses. |
| Kanban | `diagrams/kanban.rs` | `kanban` | Distinct full card/model plus terminal views; keep unknown managed metadata and full fallback fields. |
| Mindmap | `diagrams/mindmap/{db,display,terminal,render_model}.rs` | `mindmap` | Distinct full node/edge/model plus borrowed view; extract canonical types from DB and retain display policy. |
| XyChart | `diagrams/xychart.rs` | `xy_chart` | Distinct full display policy/model; terminal receives explicit policy, without theme recomputation. |
| Agentflow | `diagrams/agentflow.rs`, authored/full data in `diagrams/agentflow/presentation.rs` | `agentflow` | Distinct full visuals/model; preserve semantics distinct from Flowchart despite shared style vocabulary. |
| GitGraph | `diagrams/git_graph.rs` | `git_graph` | Same; terminal borrows canonical model; seeded command/ID construction remains core. |
| Class | `models/class_diagram.rs` | `class` | Distinct full presentation/model plus topology/view; preserve namespace facade aliases and ordered style witnesses. |

The nine distinct full-data groups are State, Gantt, Kanban, Mindmap, XY, Agentflow, Block, Treemap and Class. The six wrappers to retire are Info, Pie, Journey, Timeline, Packet and Radar. Existing terminal adapters cover fourteen payload kinds: Class, ER, Flowchart, Gantt, GitGraph, Journey, Kanban, Mindmap, Packet, Sequence, State, Timeline, TreeView and XY. Flowchart also serves the independently admitted Swimlane language.

## Symbol-level extraction constraints

| Current seam | Required treatment |
| --- | --- |
| `flowchart/model.rs` contains both public model data and parser-private `Node`, `FlowNodeSyntax`, `Edge`, `LinkToken`, `EdgeDefaults`, `TitleKind`, `LabeledText`, `SubgraphHeader` | Move data symbols, not the whole file. Keep parser-private records with grammar construction. |
| `gantt/model.rs` contains public `GanttTaskRaw` as well as private `StartTimeRaw`, `RawTaskRaw`, `RawTask`, `GanttDb`, `TaskInfo` | Public raw-task data moves with the canonical contract; private DB records and date/source resolution stay core. Names alone do not determine ownership. |
| `mindmap/db.rs` declares canonical structs before `MindmapDb` | Extract the first two public model structs. Keep node input, source scans, DB mutation, tree construction and sanitization in core. |
| `agentflow/presentation.rs` mixes authored semantics, full data and conversion to Flowchart plus render context | Move both real data groups; keep construction/preparation above them. Shared Flowchart style data does not enable a Flowchart/Swimlane parser or terminal handler. |
| `sequence/compat.rs` mixes a pure borrowed serializer and source/full wrappers | Pure `SequenceCompatibilityProjection` and placement constants belong to the data serializer; source/full parsing wrappers stay core. Do not force terminal serialization to enable full presentation. |
| State documents and record IDs are currently mutated through crate-private storage | Provide one bounded document construction/edit API with ID/root/tree invariants, preserving flat ownership and divider handling. Do not make storage public merely to bypass the new crate boundary. |
| Class precedence facts/witnesses and Flowchart subgraph style provenance use crate-private writes | Give core a deliberate data-construction interface; preserve encounter ordinals and shared storage, without JSON round trips or bool summaries. |
| Kanban full private compatibility fields include terminal fallback facts | Separate terminal-used `level`, CSS classes and shape from projection-only dimensions/padding by actual readers. Do not delete the entire private record. |
| Railroad observer scratch and public expression spans share the model file | Keep observer-only capture in core; preserve public expression `SourceSpan` data. No editor/lexer type enters the model package. |
| Managed Kanban metadata, Treemap values and ZenUML serialization | Move the required iterative clone/drop/export implementation with `ManagedJson`. A plain `Value` is not an equivalent lifecycle. |
| Usecase JSON-string helpers mix UTF-16 data encoding with token scanning | Move data codecs; keep token-end checks, ordered-source parsing and parser issue types in core. |
| Sanitizer and original-source remapping methods are attached to model types | Replace them with core family functions after moving the types. Keep the original stage order and cancellation/error channel. |
| Full wrappers have serde defaults different from canonical types | Remove and document the old full source APIs. Do not modify canonical defaults to simulate unpublished wrapper compatibility. |
| Info and other compatibility wrappers gate irrecoverable empty/presence/order state on `compat-json` | Move minimal facts to `presentation-data` and capture them unconditionally during full presentation construction. Serializer selection cannot control capture when an independent Render consumer later projects the result. |

## Projection ownership through the phases

M2 keeps source-dependent compatibility entrypoints in core while moving the pure serializers needed by typed/terminal users. M5 then shares the pure Mermaid-JSON projectors needed by both core and Render behind the model projection capability. This is an implementation sequence, not two final projection owners.

At closeout, parsing, detector effects, errors and compatibility request routing stay core; frozen values/witnesses and pure data serialization are defined once below the source/render split. State/Ishikawa/Treemap nested JSON expansion becomes controlled data projection with a local projection error, not a model method reaching into a DB. Core adapts the error at its source API. Preserve intentional compatibility duplication without recreating it in canonical model ownership.

The minimal empty/presence/order facts are `presentation-data`, not serializer code. Core full construction freezes and carries them with its family artifact even without `compat-json`; semantic-only construction need not acquire presentation work. A manual model defaults to the declared full-model output and never infers parsed-empty history from default values. If explicit projection facts are supplied, they are bounded declared data, not evidence that parsing occurred. Test a core presentation producer without JSON serialization feeding Render's projector, and preserve the distinction between parsed-empty Info and a manual `show_info: false` model.

Typed serde, complete ASCII fallback and Mermaid-compatible JSON are different projections. Cover their existing differences explicitly: Flowchart style/direction provenance, Sequence actor lifecycles, State flat documents and full direction facts, Gantt visible endpoints, Kanban unknown metadata, XY display policy and lossless Usecase JSON. Neither a unified JSON master nor a universal serialization/import service is introduced.

## Retirement and closeout checklist

- [x] All 34 canonical payloads have one model-package definition and all core grammar/DB callbacks remain internal to core.
- [x] All nine real full-data groups and their pure facts have a presentation-data owner; full metadata/context admission continues in M5.
- [ ] Full construction preserves irrecoverable projection facts without `compat-json`; independent Render projection and manual-default differences are verified before wrapper deletion.
- [x] All fourteen current terminal adapters use shared data; no extra handler is claimed by type availability.
- [ ] The six compatibility-only wrappers, old `PacketRenderBlock` name, `core::models`, public core family-model paths and facade glob export are removed after caller migration.
- [ ] State documents, Class witnesses, Flowchart provenance and Kanban sidecars have explicit construction interfaces with existing bounds.
- [ ] Source sanitizers/remappers and compatibility routing have no model-to-core edge; shared serializers do not import parser types or core errors.
- [ ] Pure and integration tests retain their proper owners; real standalone normal/build closures prove the package boundary without dev-feature unification.

Use ordinary source review, Rust compilation and the existing family/feature tests to reconcile this inventory. Do not turn the table into a custom Rust name-resolution or automatic model-discovery tool.

## Current implementation reconciliation (2026-10-30)

This section records the inspected shared worktree, including implementation changes not yet committed over `7691ed16349d1b1848eb8301d2b1dc0057684e69`. It does not identify that commit alone as the reconciled implementation. Verification for this update consisted of source reads, definition searches, public module/reexport inspection and production/test caller searches; no Cargo command ran for this reconciliation. Full final owner runs, Linux filesystem/browser evidence, package/legal checks and the final immutable implementation receipt remain M8 work.

The implemented counts match the original inventory: **34 canonical payloads, nine distinct full-data groups, fourteen terminal adapters and six removed compatibility-only wrappers**. Every canonical/full type below has one struct definition in `merman-model`; core only imports those definitions and retains its construction state. Private layout aliases in Render, such as `state.rs`'s `StateDiagramModel = merman_model::families::state::StateDiagramRenderModel`, refer to those definitions and are not additional model representations or public compatibility aliases.

### Canonical definitions: 34 of 34 reconciled

The selected enum is defined once in [model semantic.rs](../../crates/merman-model/src/semantic.rs), with family registration in [families/mod.rs](../../crates/merman-model/src/families/mod.rs). Each row below has one defining struct, rather than a core copy or a model type alias. Source paths identify the remaining parser/construction owner, not a second data definition.

| Payload | Canonical type and defining model file | Core construction owner | Status |
| --- | --- | --- | --- |
| Error | `ErrorDiagramModel` — [families/error.rs](../../crates/merman-model/src/families/error.rs) | [diagrams/error_diagram.rs](../../crates/merman-core/src/diagrams/error_diagram.rs), including source-error suppression | Reconciled |
| Flowchart | `FlowchartModel` — [families/flowchart.rs](../../crates/merman-model/src/families/flowchart.rs) | [diagrams/flowchart.rs](../../crates/merman-core/src/diagrams/flowchart.rs) and its private grammar/build modules | Reconciled; Swimlane shares data, not source admission |
| Sequence | `SequenceDiagramModel` — [families/sequence.rs](../../crates/merman-model/src/families/sequence.rs) | [sequence/db.rs](../../crates/merman-core/src/diagrams/sequence/db.rs), [sequence/parse.rs](../../crates/merman-core/src/diagrams/sequence/parse.rs) | Reconciled |
| State | `StateDiagramModel` — [state/model.rs](../../crates/merman-model/src/families/state/model.rs) | [state/db.rs](../../crates/merman-core/src/diagrams/state/db.rs), [state/ast.rs](../../crates/merman-core/src/diagrams/state/ast.rs) scratch adapter | Reconciled |
| Timeline | `TimelineDiagramModel` — [families/timeline.rs](../../crates/merman-model/src/families/timeline.rs) | [diagrams/timeline.rs](../../crates/merman-core/src/diagrams/timeline.rs) | Reconciled |
| Cynefin | `CynefinDiagramModel` — [families/cynefin.rs](../../crates/merman-model/src/families/cynefin.rs) | [diagrams/cynefin.rs](../../crates/merman-core/src/diagrams/cynefin.rs) | Reconciled |
| Info | `InfoDiagramModel` — [families/info.rs](../../crates/merman-model/src/families/info.rs) | [diagrams/info.rs](../../crates/merman-core/src/diagrams/info.rs) | Reconciled |
| Journey | `JourneyDiagramModel` — [families/journey.rs](../../crates/merman-model/src/families/journey.rs) | [diagrams/journey.rs](../../crates/merman-core/src/diagrams/journey.rs) | Reconciled; section occurrence and NaN data retained |
| Pie | `PieDiagramModel` — [families/pie.rs](../../crates/merman-model/src/families/pie.rs) | [diagrams/pie.rs](../../crates/merman-core/src/diagrams/pie.rs) | Reconciled |
| Packet | `PacketDiagramModel`, `PacketBlock` — [families/packet.rs](../../crates/merman-model/src/families/packet.rs) | [diagrams/packet.rs](../../crates/merman-core/src/diagrams/packet.rs) | Reconciled; no `PacketRenderBlock` alias |
| TreeView | `TreeViewDiagramModel` — [families/tree_view.rs](../../crates/merman-model/src/families/tree_view.rs) | [diagrams/tree_view.rs](../../crates/merman-core/src/diagrams/tree_view.rs) | Reconciled |
| QuadrantChart | `QuadrantChartDiagramModel` — [families/quadrant_chart.rs](../../crates/merman-model/src/families/quadrant_chart.rs) | [diagrams/quadrant_chart.rs](../../crates/merman-core/src/diagrams/quadrant_chart.rs) | Reconciled |
| Radar | `RadarDiagramModel` — [families/radar.rs](../../crates/merman-model/src/families/radar.rs) | [diagrams/radar.rs](../../crates/merman-core/src/diagrams/radar.rs) | Reconciled |
| Sankey | `SankeyDiagramModel` — [families/sankey.rs](../../crates/merman-model/src/families/sankey.rs) | [diagrams/sankey.rs](../../crates/merman-core/src/diagrams/sankey.rs), including CSV handling | Reconciled |
| Venn | `VennDiagramModel` — [families/venn.rs](../../crates/merman-model/src/families/venn.rs) | [diagrams/venn.rs](../../crates/merman-core/src/diagrams/venn.rs) | Reconciled |
| Requirement | `RequirementDiagramModel` — [families/requirement.rs](../../crates/merman-model/src/families/requirement.rs) | [diagrams/requirement.rs](../../crates/merman-core/src/diagrams/requirement.rs) | Reconciled |
| Usecase | `UsecaseDiagramModel` — [families/usecase.rs](../../crates/merman-model/src/families/usecase.rs), codecs in [usecase/json_string.rs](../../crates/merman-model/src/families/usecase/json_string.rs) | [diagrams/usecase.rs](../../crates/merman-core/src/diagrams/usecase.rs) and private token/source codecs | Reconciled; lossless JavaScript data remains model-owned |
| EventModeling | `EventModelingDiagramModel` — [families/event_modeling.rs](../../crates/merman-model/src/families/event_modeling.rs) | [diagrams/eventmodeling.rs](../../crates/merman-core/src/diagrams/eventmodeling.rs) | Reconciled |
| Wardley | `WardleyDiagramModel` — [families/wardley.rs](../../crates/merman-model/src/families/wardley.rs) | [diagrams/wardley.rs](../../crates/merman-core/src/diagrams/wardley.rs) | Reconciled |
| Railroad | `RailroadDiagramModel`, `RailroadAstNode` — [families/railroad.rs](../../crates/merman-model/src/families/railroad.rs) | [diagrams/railroad.rs](../../crates/merman-core/src/diagrams/railroad.rs), including dialect parsing/observer scratch | Reconciled |
| Zenuml | `ZenumlDiagramModel` — [families/zenuml.rs](../../crates/merman-model/src/families/zenuml.rs) | [zenuml/semantic.rs](../../crates/merman-core/src/diagrams/zenuml/semantic.rs), private lexer/parser/AST | Reconciled |
| Er | `ErDiagramModel` — [families/er.rs](../../crates/merman-model/src/families/er.rs) | [diagrams/er.rs](../../crates/merman-core/src/diagrams/er.rs) | Reconciled |
| Architecture | `ArchitectureDiagramModel` — [families/architecture.rs](../../crates/merman-model/src/families/architecture.rs) | [architecture/parse.rs](../../crates/merman-core/src/diagrams/architecture/parse.rs) | Reconciled |
| Gantt | `GanttDiagramModel` — [families/gantt.rs](../../crates/merman-model/src/families/gantt.rs) | [gantt/model.rs](../../crates/merman-core/src/diagrams/gantt/model.rs) DB/source records, [gantt/parse.rs](../../crates/merman-core/src/diagrams/gantt/parse.rs) | Reconciled; public `GanttTaskRaw` also model-owned |
| C4 | `C4DiagramModel` — [families/c4.rs](../../crates/merman-model/src/families/c4.rs) | [diagrams/c4.rs](../../crates/merman-core/src/diagrams/c4.rs) | Reconciled |
| Ishikawa | `IshikawaDiagramModel` — [families/ishikawa.rs](../../crates/merman-model/src/families/ishikawa.rs) | [diagrams/ishikawa.rs](../../crates/merman-core/src/diagrams/ishikawa.rs) | Reconciled |
| Treemap | `TreemapDiagramModel` — [families/treemap.rs](../../crates/merman-model/src/families/treemap.rs) | [diagrams/treemap.rs](../../crates/merman-core/src/diagrams/treemap.rs) | Reconciled; flat ownership and managed values retained |
| Block | `BlockDiagramModel` — [families/block.rs](../../crates/merman-model/src/families/block.rs) | [diagrams/block.rs](../../crates/merman-core/src/diagrams/block.rs) | Reconciled |
| Kanban | `KanbanDiagramModel` — [families/kanban.rs](../../crates/merman-model/src/families/kanban.rs) | [diagrams/kanban.rs](../../crates/merman-core/src/diagrams/kanban.rs) | Reconciled; unknown managed metadata retained |
| Mindmap | `MindmapDiagramModel` — [families/mindmap.rs](../../crates/merman-model/src/families/mindmap.rs) | [mindmap/db.rs](../../crates/merman-core/src/diagrams/mindmap/db.rs), [mindmap/parse.rs](../../crates/merman-core/src/diagrams/mindmap/parse.rs) | Reconciled |
| XyChart | `XyChartDiagramModel` — [families/xy_chart.rs](../../crates/merman-model/src/families/xy_chart.rs) | [diagrams/xychart.rs](../../crates/merman-core/src/diagrams/xychart.rs) | Reconciled |
| Agentflow | `AgentflowDiagramModel` — [families/agentflow.rs](../../crates/merman-model/src/families/agentflow.rs), [agentflow/authored.rs](../../crates/merman-model/src/families/agentflow/authored.rs) | [diagrams/agentflow.rs](../../crates/merman-core/src/diagrams/agentflow.rs), [agentflow/presentation.rs](../../crates/merman-core/src/diagrams/agentflow/presentation.rs) source preparation | Reconciled; distinct canonical semantics |
| GitGraph | `GitGraphDiagramModel` — [families/git_graph.rs](../../crates/merman-model/src/families/git_graph.rs) | [diagrams/git_graph.rs](../../crates/merman-core/src/diagrams/git_graph.rs), including seeded construction | Reconciled |
| Class | `ClassDiagramModel` — [families/class.rs](../../crates/merman-model/src/families/class.rs) | [class/db.rs](../../crates/merman-core/src/diagrams/class/db.rs), [class/parse.rs](../../crates/merman-core/src/diagrams/class/parse.rs); sanitation/tests in [class/source_data.rs](../../crates/merman-core/src/diagrams/class/source_data.rs) | Reconciled; old core `models` tree deleted |

### Full presentation groups: nine of nine reconciled

All nine real full types are selected by model `presentation-data`, independently of `compat-json`. The single full enum is [presentation/diagram.rs](../../crates/merman-model/src/presentation/diagram.rs). Frozen metadata, family context and checked pairing are owned by [presentation/config.rs](../../crates/merman-model/src/presentation/config.rs), [context.rs](../../crates/merman-model/src/presentation/context.rs) and [input.rs](../../crates/merman-model/src/presentation/input.rs); core's [ParsedDiagramRender](../../crates/merman-core/src/presentation/diagram.rs) privately contains the shared `PresentationDiagram` and retains the source-origin guarantee.

| Group | Single full-data owner | Preserved data and explicit construction seam | Status |
| --- | --- | --- | --- |
| State | `StateDiagramRenderModel`, nodes/edges — [state/render_model.rs](../../crates/merman-model/src/families/state/render_model.rs) | Shares the validated flat `StateDocument`; core [state/render_model.rs](../../crates/merman-core/src/diagrams/state/render_model.rs) constructs full node/edge facts | Reconciled |
| Gantt | `GanttDiagramRenderModel`, `GanttRenderTask`, `GanttOutputData` — [families/gantt.rs](../../crates/merman-model/src/families/gantt.rs) | Canonical task plus visible/render endpoints and display mode; `from_output_data` preserves empty/model distinction | Reconciled |
| Kanban | `KanbanDiagramRenderModel`, `KanbanRenderNode`, `KanbanRenderNodeCompatibility` — [families/kanban.rs](../../crates/merman-model/src/families/kanban.rs) | `new`, `with_compatibility`, read-only `compatibility`; terminal-used level/CSS/shape and projection dimensions remain distinct | Reconciled |
| Mindmap | Full model/nodes/edges — [mindmap/presentation.rs](../../crates/merman-model/src/families/mindmap/presentation.rs) | [display.rs](../../crates/merman-model/src/families/mindmap/display.rs) owns pure policy; [identity.rs](../../crates/merman-model/src/families/mindmap/identity.rs) derives ID from explicit runtime context; `PresentationContext::Mindmap` retains explicit output ID | Reconciled; source tree/DB construction remains core |
| XyChart | `XyChartDiagramRenderModel`, `XyChartDisplayPolicy` — [families/xy_chart.rs](../../crates/merman-model/src/families/xy_chart.rs) | Canonical model plus explicit display policy; no source theme evaluator moved into the data type | Reconciled |
| Agentflow | `AgentflowDiagramRenderModel`, `AgentflowVisuals`, label sources — [agentflow/presentation.rs](../../crates/merman-model/src/families/agentflow/presentation.rs) | `from_node_dom_indices` freezes DOM ordinals; authored styles remain in [authored.rs](../../crates/merman-model/src/families/agentflow/authored.rs); source conversion stays core | Reconciled |
| Block | `BlockDiagramRenderModel`, `BlockVisuals`, `BlockProjectionFacts` — [families/block.rs](../../crates/merman-model/src/families/block.rs) | Canonical graph plus palette and absent/null/value facts; checked family projection reads borrowed declarations | Reconciled |
| Treemap | `TreemapDiagramRenderModel`, `TreemapVisuals`, `TreemapOutputPresence` — [families/treemap.rs](../../crates/merman-model/src/families/treemap.rs) | Flat canonical data plus visuals and declared output presence; [compat::PreparedProjection](../../crates/merman-model/src/families/treemap/compat.rs) keeps private checked inputs and the existing 8 MiB projection cap | Reconciled |
| Class | `ClassDiagram`, `ClassPresentation`, render views/topology — [families/class.rs](../../crates/merman-model/src/families/class.rs) | `new_controlled`, `ClassStylePrecedenceFacts::from_witnesses`, witness constructors and `validate_for_model_controlled`; style observation/sanitization remain core | Reconciled |

The additional Flowchart context is defined once in [flowchart/presentation.rs](../../crates/merman-model/src/families/flowchart/presentation.rs). `FlowchartSubgraphStyles::insert`, label-source insertions, `FlowchartRenderContext::new`/DOM-index construction and controlled relationship validators replace former private writes. Duplicate source IDs, hidden/collapsed nodes and actual style encounter ordinals remain data, not inferred parser authentication. State's `StateDocumentBuilder::{append,edit,build_controlled}` in [state.rs](../../crates/merman-model/src/families/state.rs) validates completed references/root/tree relationships while core retains `StateDocumentScratch` and observers.

### Terminal adapters: fourteen of fourteen reconciled

[AsciiDiagram](../../crates/merman-ascii/src/model.rs) has exactly the fourteen family adapter variants below, plus `Unsupported`; ASCII handler capability belongs to [capability.rs](../../crates/merman-ascii/src/capability.rs). No ASCII production handler admission queries core parser availability. Full-host view branches require `presentation-data` where applicable; this does not enable another parser or handler.

| Payload | Borrowed data / complete fallback owner | Terminal consumer | Status |
| --- | --- | --- | --- |
| Class | `ClassTopologyView` and full topology/render views — [families/class.rs](../../crates/merman-model/src/families/class.rs) | [class/render.rs](../../crates/merman-ascii/src/class/render.rs) | Reconciled; authored model borrowed, derived topology admitted |
| ER | `ErDiagramModel` — [families/er.rs](../../crates/merman-model/src/families/er.rs) | [er/render.rs](../../crates/merman-ascii/src/er/render.rs) | Reconciled; direct borrow |
| Flowchart / Swimlane | `FlowchartModel`, provenance, styles and explicit directions — [families/flowchart.rs](../../crates/merman-model/src/families/flowchart.rs); fallback augmentation in [ASCII model.rs](../../crates/merman-ascii/src/model.rs) | [graph/adapter.rs](../../crates/merman-ascii/src/graph/adapter.rs) | Reconciled; one payload, separately selected languages |
| Gantt | `GanttTerminalView` — [gantt/terminal.rs](../../crates/merman-model/src/families/gantt/terminal.rs) | [gantt.rs](../../crates/merman-ascii/src/gantt.rs) | Reconciled; canonical/full endpoint branches retained |
| GitGraph | `GitGraphDiagramModel` — [families/git_graph.rs](../../crates/merman-model/src/families/git_graph.rs) | [git_graph.rs](../../crates/merman-ascii/src/git_graph.rs) | Reconciled; direct borrow |
| Journey | `JourneyDiagramModel`, section occurrence/NaN witnesses — [families/journey.rs](../../crates/merman-model/src/families/journey.rs) | [journey.rs](../../crates/merman-ascii/src/journey.rs) | Reconciled; direct canonical borrow |
| Kanban | `KanbanTerminalView` — [families/kanban.rs](../../crates/merman-model/src/families/kanban.rs) | [kanban.rs](../../crates/merman-ascii/src/kanban.rs) | Reconciled; canonical/full and unknown managed metadata retained |
| Mindmap | `MindmapTerminalView` — [mindmap/terminal.rs](../../crates/merman-model/src/families/mindmap/terminal.rs), [display.rs](../../crates/merman-model/src/families/mindmap/display.rs) | [mindmap.rs](../../crates/merman-ascii/src/mindmap.rs) | Reconciled; canonical/full branches retained |
| Packet | `PacketDiagramModel` / `PacketBlock` — [families/packet.rs](../../crates/merman-model/src/families/packet.rs) | [packet.rs](../../crates/merman-ascii/src/packet.rs) | Reconciled; validation stays with the data |
| Sequence | `SequenceCompatibilityProjection` — [sequence/compat.rs](../../crates/merman-model/src/families/sequence/compat.rs), lifecycle vocabulary in [sequence.rs](../../crates/merman-model/src/families/sequence.rs) | [sequence.rs](../../crates/merman-ascii/src/sequence.rs), complete fallback in [model.rs](../../crates/merman-ascii/src/model.rs) | Reconciled; borrowed serializer works without full presentation or `compat-json`; fallback adds lifecycle records |
| State | `StateTerminalView` — [state/terminal.rs](../../crates/merman-model/src/families/state/terminal.rs) | [state/adapter.rs](../../crates/merman-ascii/src/state/adapter.rs) | Reconciled; flat statements and canonical/full nodes/edges retained |
| Timeline | `TimelineDiagramModel` — [families/timeline.rs](../../crates/merman-model/src/families/timeline.rs) | [timeline.rs](../../crates/merman-ascii/src/timeline.rs) | Reconciled; direct canonical borrow |
| TreeView | `TreeViewDiagramModel` — [families/tree_view.rs](../../crates/merman-model/src/families/tree_view.rs) | [tree_view.rs](../../crates/merman-ascii/src/tree_view.rs) | Reconciled; direct canonical borrow |
| XY | `XyChartDiagramModel` plus explicit `XyChartDisplayPolicy` — [families/xy_chart.rs](../../crates/merman-model/src/families/xy_chart.rs) | [xychart/mod.rs](../../crates/merman-ascii/src/xychart/mod.rs) | Reconciled; no theme recomputation |

### Compatibility-only wrappers: six of six removed

The old wrapper type names below have no remaining definition, alias or Rust caller in the inspected crates. The canonical payload now appears directly in `RenderSemanticModel`; `PresentationContext` carries the minimal presence declaration. Every output enum is model `presentation-data`, defaults to `Model`, and is captured by core full construction selected by `presentation`, rather than by the serializer feature. [Model builtin projectors](../../crates/merman-model/src/presentation/builtin.rs) consume the paired context explicitly. This source reconciliation is not a replacement for the final no-JSON-producer/independent-Render execution receipt.

| Removed type | Canonical payload and presence owner | Core capture and shared projection | Status |
| --- | --- | --- | --- |
| `InfoDiagramRenderModel` | `InfoDiagramModel`, `InfoOutputData` — [info.rs](../../crates/merman-model/src/families/info.rs) | [core info.rs](../../crates/merman-core/src/diagrams/info.rs), [info/compat.rs](../../crates/merman-model/src/families/info/compat.rs) | Removed; parsed-empty and manual `show_info: false` are distinct declared shapes |
| `PieDiagramRenderModel` | `PieDiagramModel`, `PieOutputData` — [pie.rs](../../crates/merman-model/src/families/pie.rs) | [core pie.rs](../../crates/merman-core/src/diagrams/pie.rs), [pie/compat.rs](../../crates/merman-model/src/families/pie/compat.rs) | Removed; empty/model presence retained |
| `JourneyDiagramRenderModel` | `JourneyDiagramModel`, `JourneyOutputData` — [journey.rs](../../crates/merman-model/src/families/journey.rs) | [core journey.rs](../../crates/merman-core/src/diagrams/journey.rs), [journey/compat.rs](../../crates/merman-model/src/families/journey/compat.rs) | Removed; section order, repeated sections and NaN witnesses retained |
| `TimelineDiagramRenderModel` | `TimelineDiagramModel`, `TimelineOutputData` — [timeline.rs](../../crates/merman-model/src/families/timeline.rs) | [core timeline.rs](../../crates/merman-core/src/diagrams/timeline.rs), [timeline/compat.rs](../../crates/merman-model/src/families/timeline/compat.rs) | Removed; empty/model presence and section occurrences retained |
| `PacketDiagramRenderModel` | `PacketDiagramModel`, `PacketOutputData`, `PacketBlock` — [packet.rs](../../crates/merman-model/src/families/packet.rs) | [core packet.rs](../../crates/merman-core/src/diagrams/packet.rs), [packet/compat.rs](../../crates/merman-model/src/families/packet/compat.rs) | Removed; no old `PacketRenderBlock` alias |
| `RadarDiagramRenderModel` | `RadarDiagramModel`, `RadarOutputData` — [radar.rs](../../crates/merman-model/src/families/radar.rs) | [core radar.rs](../../crates/merman-core/src/diagrams/radar.rs), [radar/compat.rs](../../crates/merman-model/src/families/radar/compat.rs) | Removed; JSON numeric payloads retained |

### Shared contracts and source retirement

| Retired seam | Current defining owner / remaining responsibility | Reconciled status |
| --- | --- | --- |
| Public `core::diagrams::*`, `core::models::*` and core-root shared payload/metadata aliases | [core lib.rs](../../crates/merman-core/src/lib.rs) makes grammar/construction modules private; shared data is imported from model; `models` files are deleted | Retired; no old core namespace caller found in crate Rust sources |
| Public core config/operation/runtime/resource/text/JSON/time/markup forwarding | [model lib.rs](../../crates/merman-model/src/lib.rs) and its defining modules; [markup lib.rs](../../crates/merman-markup/src/lib.rs); core uses ordinary private imports | Retired; forwarding files deleted; source TLS remains private [runtime.rs](../../crates/merman-core/src/runtime.rs) |
| Core resources adapter and pure statistics copies | [model resources.rs](../../crates/merman-model/src/resources.rs), [resources/data.rs](../../crates/merman-model/src/resources/data.rs), [resources/presentation.rs](../../crates/merman-model/src/resources/presentation.rs); family-owned Sequence/Treemap/Kanban/Block statistics are reexported there | Retired; core has only [resource_tests.rs](../../crates/merman-core/src/resource_tests.rs) under `cfg(test)`, no production resource module/adapter |
| Source-policy methods on `MermaidConfig` | Private `authored_source_overrides` / `source_filtered_overrides` in [core config/mod.rs](../../crates/merman-core/src/config/mod.rs) | Reconciled; model value container has no source sanitizer edge |
| Model sanitation and original-source remapping | Family free functions in core; Class moved to [class/source_data.rs](../../crates/merman-core/src/diagrams/class/source_data.rs); source remapping remains in [presentation/diagram.rs](../../crates/merman-core/src/presentation/diagram.rs) and parse pipeline | Reconciled; no model/markup import of core parser errors or source types found |
| Core-owned pure compatibility serializers / DB shallow projection helper | One projector per model `families/<family>/compat.rs`, dispatched by [presentation/builtin.rs](../../crates/merman-model/src/presentation/builtin.rs); [compat.rs](../../crates/merman-model/src/presentation/compat.rs) owns borrowed metadata and local errors | Reconciled; source wrappers retain routing/error adaptation; State shallow helper remains private to its projector |
| Managed JSON/lifecycle duplicates | [model json/managed.rs](../../crates/merman-model/src/json/managed.rs); family iterative serde/clone/drop remain with their canonical owner | Reconciled; no universal JSON import bridge or second successful model representation |
| Sanitizer/URL/structural CSS/Khroma duplicate owners | [markup sanitize.rs](../../crates/merman-markup/src/sanitize.rs), [sanitize/fragment.rs](../../crates/merman-markup/src/sanitize/fragment.rs), [url.rs](../../crates/merman-markup/src/url.rs), [style.rs](../../crates/merman-markup/src/style.rs), [theme_color.rs](../../crates/merman-markup/src/theme_color.rs) | Reconciled; shared sink contract is markup-owned; Render's [html.rs](../../crates/merman-render/src/html.rs) retains its concrete budget adapter and output CSS policy |
| Core theme catalog, numeric iteration coercion and duplicate ECMAScript whitespace | [config/theme_id.rs](../../crates/merman-model/src/config/theme_id.rs), [presentation/theme.rs](../../crates/merman-model/src/presentation/theme.rs), [text/whitespace.rs](../../crates/merman-model/src/text/whitespace.rs) | Reconciled; source evaluator stays core; no core `ThemeEvaluationLimitExceeded` alias |
| Facade `pub use merman_core::*` / pure selector forwarding | Explicit workflow exports and `merman::model` in [facade lib.rs](../../crates/merman/src/lib.rs); selectors use model [family.rs](../../crates/merman-model/src/family.rs) | Retired; source theme UI/header/provider queries deliberately remain source APIs |
| Eager fixes/config retention in canonical analysis candidates | [analysis analyzer.rs](../../crates/merman-analysis/src/analyzer.rs) request policy, immutable evidence and projection-local materialization in [repair.rs](../../crates/merman-analysis/src/repair.rs) | Reconciled; `config_for_rewrite` absent; this table does not claim final product/fixes matrix execution |

### Private-export inventory: source/model bridge retired

The old core `presentation::engine::__private` module and root reexport are absent. The table accounts for its exported symbols individually; canonical types have ordinary shared public APIs, while actual source installation remains a source API. A private module alias inside core is an implementation import, not a replacement public bridge.

| Former export / adapter | Current owner or direct replacement | Status / production callers |
| --- | --- | --- |
| `ThemeCompatibilityRecipe` wrapping `ThemeParseBinding` | `ThemeCompatibilityRecipe` / `ThemeCompatibilityRecipeError` — [model recipe.rs](../../crates/merman-model/src/presentation/recipe.rs) | Shared recipe only; old binding type/alias absent |
| `ThemeCompatibilityPlan` | [core theme_plan.rs](../../crates/merman-core/src/presentation/theme_plan.rs), reexported by ordinary `core::presentation` and core root | Retained source contract; owns initialization/cache only |
| `ThemeCompatibilityPlanError` | [core theme_plan.rs](../../crates/merman-core/src/presentation/theme_plan.rs), wrapping shared recipe error | Retained source error; no private bridge |
| `install_theme_compatibility` | `MermaidEngine::with_theme_plan` — [core engine.rs](../../crates/merman-core/src/presentation/engine.rs) | Old public helper retired; actual install is a private plan method |
| `GitGraphPaintInput`, `GitGraphPaintInputs` | [model paint.rs](../../crates/merman-model/src/presentation/paint.rs) | One definition; consumers import shared types |
| `ErPaintInput`, `ErPaintInputs` | [model paint.rs](../../crates/merman-model/src/presentation/paint.rs) | One definition; consumers import shared types |
| `gitgraph_paint_inputs` | `PresentationConfig::gitgraph_paint_inputs` — [model config.rs](../../crates/merman-model/src/presentation/config.rs) | Forwarder retired |
| `er_paint_inputs` | `PresentationConfig::er_paint_inputs` — [model config.rs](../../crates/merman-model/src/presentation/config.rs) | Forwarder retired |
| `ThemeCompatibilityFieldKind` | [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | One definition |
| `ThemeCompatibilityFieldEvidence` | `ThemeCompatibilityField` — [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | Old name/alias retired; checked field constructor retained |
| `ThemeCompatibilityConsumptionDisposition` | [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | One definition |
| `ThemeCompatibilityFieldConsumption` | [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | One definition |
| `ThemeCompatibilityReconciliation` | [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | One definition |
| `ThemeParseEvidence` | `ThemeCompatibilityEvidence` — [model ownership.rs](../../crates/merman-model/src/presentation/ownership.rs) | Old name/alias retired |
| `theme_parse_matches_recipe` | `config.theme_compatibility().matches_recipe(...)` | Forwarder retired; complete normalized recipe comparison remains shared |
| `theme_parse_mermaid_residual_count` | `config.theme_compatibility().mermaid_residual_count()` | Forwarder retired |
| `theme_parse_evidence` | Borrow `config.theme_compatibility()` | Cloning bridge retired |
| `config_path_overrides_typed_default` | Direct `PresentationConfig` method — [model config.rs](../../crates/merman-model/src/presentation/config.rs) | Forwarder retired |
| `config_paint_overrides_typed_default` | Direct `PresentationConfig` method — [model config.rs](../../crates/merman-model/src/presentation/config.rs) | Forwarder retired |
| `explicit_config_owns_path` | Direct `PresentationConfig` method — [model config.rs](../../crates/merman-model/src/presentation/config.rs) | Forwarder retired |
| `theme_color_iterations` | Shared function and `ThemeEvaluationLimitExceeded` — [model theme.rs](../../crates/merman-model/src/presentation/theme.rs) | Numeric bridge and old core error alias retired; theme orchestration stays core |
| Render `DiagramTheme::install_parse_compatibility` production bridge | `DiagramTheme::compatibility_recipe()` + core `ThemeCompatibilityPlan::from_recipe` + `with_theme_plan` | Production bridge retired; only crate-private `cfg(test)` convenience remains in [diagram_theme/mod.rs](../../crates/merman-render/src/diagram_theme/mod.rs) |

### Private-export inventory: retained output consumers

[Render lib.rs](../../crates/merman-render/src/lib.rs) still declares `merman_render::__private`. These are renderer/output-owned seams with actual consumers, not source parsing, model/config forwarding or producer authentication. They are recorded as **retained**, not silently counted as deleted M7 bridges. No new model/markup `__private` module was introduced.

| Retained symbol(s) | Actual responsibility / caller | Status |
| --- | --- | --- |
| `EffectComposition`, `EffectColorSpace`, `EffectInput` | Effect vocabulary used by [export native_filter_receipt.rs](../../crates/merman-export/src/native_filter_receipt.rs) | Retained output contract |
| `NativeSvgFilterApplication`, `NativeSvgFilterReceipt`, `NativeSvgFilterUnits`, `NativeSvgShadowStage`, `MAX_NATIVE_SHADOW_STAGES` | Exact terminal native-filter receipt consumed by export and facade [render/evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained output contract |
| `FamilyEvidenceStatus`, `FamilyEvidenceSummary`, `family_evidence` | Bounded final family evidence for [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained; no private mechanism ledger exported |
| `RootEvidenceSummary`, `root_evidence` | Bounded final root evidence for [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained output projection |
| `into_session_and_theme_diagnostics` | Transfers completed renderer session/diagnostics to [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained output transfer |
| `standalone_admission_summary` | Target admission reads final SVG facts in [facade render.rs](../../crates/merman/src/render.rs) | Retained output projection |
| `root_applied_capabilities` | Final writer's proved capability set for [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained output projection |
| `family_native_filter_receipt` | Final filter receipt for [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained output projection |
| `effective_theme_resource_policy` | Captured policy intersection for [facade evidence.rs](../../crates/merman/src/render/evidence.rs) | Retained renderer policy query |
| `PreparedMathEvidenceSummary`, `prepared_math_evidence` | Sealed SVG math evidence for [target_admission/mod.rs](../../crates/merman/src/render/target_admission/mod.rs) | Retained output projection |
| `unsupported_native_html_style`, `unsupported_native_text_style` | Target-specific style rejection in [facade render.rs](../../crates/merman/src/render.rs) | Retained Render token policy |
| `finalize_standalone_for_target_admission`, `finalize_resvg_for_target_admission` | Renderer finalization with target-owned admission in [facade render.rs](../../crates/merman/src/render.rs) | Retained output finalization; source authority not transferred |
| `svg_text_fonts_are_self_contained` | Renderer font seal queried by [target_admission/mod.rs](../../crates/merman/src/render/target_admission/mod.rs) | Retained output query |
| `ArchitectureTextReceipt`, `ArchitectureTextRole`, `ArchitectureTextTerminal`, `architecture_text_receipt` | Terminal Architecture qualification under `merman_internal_theme_acceptance` + `layout-cytoscape` | Retained, acceptance-only export gate |
| Facade `__theme_acceptance` | [theme_acceptance.rs](../../crates/merman/src/theme_acceptance.rs), gated by `svg` + workspace acceptance cfg | Retained qualification owner; not a shared data/source bridge |

### Caller migration and evidence limits

No obsolete core public shared namespace, canonical/full wrapper alias or production caller of the retired source/theme bridge was found in the inspected crate sources. Render/ASCII private aliases and the retained output contracts above remain ordinary consumers of one defining type.

The two **test-only** callers previously naming the removed `merman_render::__private::install_parse_compatibility` have been migrated by the parent and inspected in the current source:

- [block_title_legacy_projection.rs](../../crates/merman-theme-acceptance/tests/block_title_legacy_projection.rs), in the legacy projection comparison loop.
- [flowchart_marker_legacy_projection.rs](../../crates/merman-theme-acceptance/tests/flowchart_marker_legacy_projection.rs), in the marker projection comparison loop.

Both now construct the core source `ThemeCompatibilityPlan` from `theme.compatibility_recipe().clone()` and call `engine.with_theme_plan(&plan)` through the facade's source API exports, matching renderer [test_support.rs](../../crates/merman-render/src/test_support.rs) and integration [tests/common/mod.rs](../../crates/merman-render/tests/common/mod.rs). No old bridge call remains in the inspected theme-acceptance Rust sources, and no Render-to-core production helper was restored. Other `install_parse_compatibility` spellings in renderer test modules/common helpers are test-local convenience functions using that final source contract. `RenderError::ThemeParseBindingMismatch` is an actual recipe-mismatch diagnostic, not a surviving `ThemeParseBinding` data alias. These source migrations still await compilation and execution with the acceptance cfg enabled.

The parent's subsequent workspace all-target reconciliation found and updated additional consumers outside this inventory worker's implementation scope:

| Consumer | Inspected current adaptation | Ownership / evidence status |
| --- | --- | --- |
| Rustdoc production resource limits and test fixtures | [config.rs](../../crates/merman-cli/src/rustdoc/config.rs), [document.rs](../../crates/merman-cli/src/rustdoc/document.rs), [publication.rs](../../crates/merman-cli/src/rustdoc/publication.rs), [receipt.rs](../../crates/merman-cli/src/rustdoc/receipt.rs), [svg.rs](../../crates/merman-cli/src/rustdoc/svg.rs) use `merman::model::resources` | Parent-owned consumer migration; no resource forwarding alias restored |
| Tree-sitter conformance source config and typed identity | [conformance.rs](../../distribution/tree-sitter-mermaid/tests/conformance.rs) uses source `SourcePresentationConfig` and `merman_model::family::diagram_type_metadata_id`; [Cargo.toml](../../distribution/tree-sitter-mermaid/Cargo.toml) declares the direct model dev dependency | Parent-owned source/test consumer migration; grammar and scanner ownership unchanged |
| UniFFI text measurement callback fixture | [lib.rs](../../crates/merman-uniffi/src/lib.rs) handles `HtmlLineHeight` in the height-result branch | Parent-owned test coverage adaptation; no shared-data alias or source bridge introduced |
| Theme-acceptance source configuration and family identity | The two projection tests above, [class_edge_label_background_legacy_projection.rs](../../crates/merman-theme-acceptance/tests/class_edge_label_background_legacy_projection.rs), and [preset_qualification.rs](../../crates/merman-theme-acceptance/src/preset_qualification.rs) use source `SourcePresentationConfig` and canonical `merman::model::family::DiagramFamilyId` imports where needed | Parent-owned consumer migration; source inspected, acceptance-cfg compilation and execution pending |

These rows record inspected source changes, not a Cargo result from this inventory worker. The parent reports that the workspace all-target check passed under the default cfg, which does not enable `merman_internal_theme_acceptance`; that result does not establish acceptance-cfg compilation or execution. Product nextest runs are still in progress at this handoff, with no final result recorded here. These observations supplement the canonical-owner inventory rather than establishing exhaustive product-test completion.

The Tree-sitter dev dependency also changes the `Cargo.lock` path-package dependency list. The current registry `(name, version, checksum)` set was compared with the prepared Linux cache receipt and still matches all **568** cached archives. The lock-file SHA256 itself has changed: reuse the archive cache, then recheck the final frozen lock and update its fingerprint receipt before Linux owners run. This provisional reconciliation does not replace the final lock receipt.

The source ownership tables and the identified acceptance caller migrations are reconciled; acceptance-cfg validation remains pending. The historical unchecked boxes above are not changed into test-pass claims. Final M8 evidence must still be recorded against the frozen source: ordinary feature/singleton owners, independent normal/build dependency closures, presentation-without-JSON producers, deep lifecycle/cancellation, Linux filesystem/browser owners, legal/package contents and product matrices. Prepared Linux tools or a successful production check alone do not close those acceptance cases.
