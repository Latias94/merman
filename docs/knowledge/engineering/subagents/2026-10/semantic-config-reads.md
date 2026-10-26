---
type: "Subagent Finding"
title: "Semantic configuration reads before the capability split"
description: "Subagent Finding for Semantic configuration reads before the capability split."
timestamp: 2026-10-20T10:50:40Z
record_id: "c47ee8b9e6874ddb83a507cecc92548f"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
subagent_id: "/root/semantic_config_reads"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "eb62a3863"
---

# Finding

Read-only U5 audit of builtin canonical construction and compatibility projection. Family owners
have no direct reads of calculated `themeVariables`; that does not permit replacing admitted
configuration with defaults. Preserve semantic normalization, authored selectors and validation.
No implementation, builds or benchmark claims came from this audit.

# Evidence

A = required semantic/admission behavior. B = presentation or compatibility projection.
C = currently mixed with typed fields; preserve behavior until the boundary is explicitly split.
Paths below are relative to `crates/merman-core/src/diagrams/`.

| Owner | Configuration and behavior | Class | Source |
| --- | --- | --- | --- |
| Common fields | Sanitized title/accessibility; ER subgraph titles and Railroad rule/AST text | A | `parse_pipeline.rs` common field sanitation; family `sanitize_common_db_fields` |
| Flowchart/Swimlane | `maxEdges`, `flowchart.inheritDir`, sanitized labels, URLs and callbacks | A | `flowchart.rs`, `flowchart/semantic.rs` |
| Flowchart | SVG label alternatives are paired context; style/class/shape routing order is also ASCII input | A/B | `flowchart/semantic.rs`, family construction |
| Agentflow | `maxEdges` uses `as_f64`, unlike Flowchart `as_u64`; `flowchart.inheritDir`; metadata title | A | `agentflow.rs` `push_edge`, `close_context`, model construction |
| Agentflow | Tooltip/link/callback values are presentation, but style creates unknown nodes and linkStyle validates indices | A/B | `agentflow/presentation.rs` |
| Class | Sanitize class IDs before generic splitting and identity merge; normalize URL/target/callback | A | `class/db.rs` |
| Class | Namespace IDs currently only trim/qualify; do not add sanitation while moving ownership | A | `class/db.rs` namespace lookup |
| Class | `class.hierarchicalNamespaces` changes typed topology, membership and aliases | C | `class/db.rs` `into_typed_model` |
| Sequence | Root `wrap` takes precedence over `sequence.wrap`; captured flags and source wrap/nowrap text behavior | C | `sequence/parse.rs` `sequence_wrap_enabled`, `sequence/db.rs` |
| State | Sanitized descriptions affect shape, label splitting and group-description rejection | A | `state/db.rs` `apply_state_descriptions`, group validation |
| State | `look` changes typed `arrow_type_end` and compatibility projection | C | `state/db.rs`, `state/render_model.rs` |
| State | `state.wrappingWidth`, `state.minNodeWidth`, `flowchart.padding` decorate dimensions | B | `state/db.rs` semantic layout-data builder |
| Kanban | Sanitized IDs precede empty-ID generation/parent assignment; shape label is sanitized after override | A | `kanban.rs` construction/projection |
| Kanban | `mindmap.padding` is decorative, but checked doubling overflow is existing parse admission | A/C | `kanban.rs` configuration arithmetic |
| Kanban | `mindmap.maxNodeWidth`, `look` compatibility decoration | B | `kanban.rs` |
| Mindmap | Sanitized node IDs/descriptions; padding and maxNodeWidth captured during construction | C | `mindmap/db.rs` |
| Mindmap | Raw theme string containing redux selects default shape; `look` and `mindmap.colorBy` affect typed fields | C | `mindmap/db.rs`, `mindmap/parse.rs` |
| GitGraph | `gitGraph.mainBranchName`, `mainBranchOrder`, `seed`; sanitized commit/branch identity before lookup | A | `git_graph.rs` |
| Packet | `packet.bitsPerRow` validates and constructs packet rows/padding | A | `packet.rs` |
| Gantt | `securityLevel` normalizes URL/effective click action; clickable class still gets added | A | `gantt/parse.rs`, `gantt/model.rs` |
| Gantt | `gantt.displayMode`, overridable by source directive | B | `gantt/model.rs` |
| C4 | Root `wrap` captured per source object; renderer later combines c4.wrap | C | `c4.rs` |
| XYChart | `xyChart.chartOrientation`, source override and sanitized axes/categories/point labels | C | `xychart.rs` |
| XYChart | showTitle/dataLabel and axis showLabel/title/tick/line display policy | B | `xychart.rs` `XyChartDisplayPolicy` |
| Sankey | Sanitized identity precedes node-map lookup/insert and endpoint merging | A | `sankey.rs` |
| Block, Quadrant, EventModeling, Ishikawa, TreeView, Railroad | Existing label/description/entity/rule/AST sanitation | A | Respective family semantic owners |
| Architecture, Pie, Journey, Timeline, Requirement, Radar, Treemap, Cynefin, Venn, Wardley, Usecase | Common-field sanitation; Usecase also consumes metadata title | A | Family owners; `usecase/resolve.rs` |
| Zenuml, Info, Error | No extra configuration decisions; preserve deliberate Zenuml language semantics and error/suppression identity | A | `zenuml/model.rs`, `info.rs`, `error_diagram.rs` |
| Compatibility JSON owners | Whole effective configuration export does not require lean materialization | B | Architecture, Block, C4, GitGraph, Kanban, Mindmap, Packet, Quadrant, Radar, Requirement, Sankey, State, Treemap, XYChart |
| Architecture/Mindmap compatibility | Authored `meta.config.layout` presence selects default layout | B | `architecture.rs`, `mindmap/render_model.rs` |

The shared sanitizer reads `securityLevel`, root `htmlLabels` before `flowchart.htmlLabels`, and
`dompurifyConfig`: ALLOWED/ADD/FORBID_TAGS, ALLOWED/ADD/FORBID_ATTR, ADD_URI_SAFE_ATTR,
ADD_DATA_URI_TAGS, ALLOW_ARIA_ATTR, ALLOW_DATA_ATTR, ALLOW_UNKNOWN_PROTOCOLS, KEEP_CONTENT,
FORBID_CONTENTS, ADD_FORBID_CONTENTS and SAFE_FOR_XML (`sanitize.rs`). These values can change
identity and labels, despite their visual names.

# Recommendation

Retain raw/admitted configuration and cheap selectors in the semantic owner; omit calculated
visual programs. Preserve sanitizer timing and collision behavior, side effects of authored
presentation directives, Kanban arithmetic rejection, State group/note/generated-ID validation,
and authored style precedence for ASCII. Mindmap's redux shape choice is not theme calculation.
Mindmap currently doubles padding directly, unlike Kanban's checked multiplication; do not
silently alter that separate behavior during the split.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md` U5/U6, R3/R7/R8.
- `docs/knowledge/engineering/subagents/2026-10/semantic-boundary-seams.md`.
- `crates/merman-core/src/sanitize.rs`, `parse_pipeline.rs`, and family owners listed above.
