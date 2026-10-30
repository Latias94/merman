# Family data migration inventory

Status: planning checklist, 2026-10-30. Source: `8888358b1873c89049c0e430615c91d406dac224`. Implementation is not complete. This inventory supports [Model, Source, and Consumer Boundaries](2026-10-30-model-and-source-boundaries.md); it is not a new machine input or a source-analysis gate.

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

- [ ] All 34 canonical payloads have one model-package definition and all core grammar/DB callbacks remain internal to core.
- [ ] All nine real full-data groups and their pure facts have a final presentation-data owner.
- [ ] Full construction preserves irrecoverable projection facts without `compat-json`; independent Render projection and manual-default differences are verified before wrapper deletion.
- [ ] All fourteen current terminal adapters use shared data; no extra handler is claimed by type availability.
- [ ] The six compatibility-only wrappers, old `PacketRenderBlock` name, `core::models`, public core family-model paths and facade glob export are removed after caller migration.
- [ ] State documents, Class witnesses, Flowchart provenance and Kanban sidecars have explicit construction interfaces with existing bounds.
- [ ] Source sanitizers/remappers and compatibility routing have no model-to-core edge; shared serializers do not import parser types or core errors.
- [ ] Pure and integration tests retain their proper owners; real standalone normal/build closures prove the package boundary without dev-feature unification.

Use ordinary source review, Rust compilation and the existing family/feature tests to reconcile this inventory. Do not turn the table into a custom Rust name-resolution or automatic model-discovery tool.
