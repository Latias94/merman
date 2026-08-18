; Portable tags describe stable named declarations. Narrative-only families
; stay explicitly not applicable rather than receiving synthetic diagram tags.

; Architecture.
(architecture_group_statement
  id: (architecture_identifier) @name) @definition.module

[
  (architecture_service_statement
    id: (architecture_identifier) @name)
  (architecture_junction_statement
    id: (architecture_identifier) @name)
] @definition.class

; Block.
(block_node
  id: (block_identifier) @name
  shape: (_)) @definition.var

; C4.
(c4_boundary_statement
  id: (c4_reference
    value: (c4_identifier) @name)) @definition.module

(c4_entity_declaration
  id: (c4_reference
    value: (c4_identifier) @name)) @definition.class

; Class.
(class_namespace_declaration
  name: (class_namespace_name
    (identifier) @name)) @definition.module

(class_declaration
  name: (class_name
    (identifier) @name)) @definition.class

; Entity Relationship.
(er_entity_declaration
  name: (er_entity_name) @name) @definition.class

; Event Modeling.
(event_entity_statement
  name: (event_qualified_name) @name) @definition.class

(event_data_statement
  name: (event_data_name) @name) @definition.var

; Flowchart.
(flow_subgraph
  id: (flow_node_id) @name) @definition.module

(flow_vertex
  id: (flow_node_id) @name
  shape: (_)) @definition.var

; Gantt.
(gantt_section_statement
  name: (gantt_line_text) @name) @definition.module

(gantt_task_statement
  metadata: (gantt_task_metadata
    (gantt_task_item
      value: (gantt_task_atom) @name))) @definition.function

; GitGraph.
(git_graph_branch_statement
  name: (git_graph_reference) @name) @definition.module

; Journey.
(journey_section_statement
  section: (journey_section_name) @name) @definition.module

(journey_task_statement
  task: (journey_task_name) @name) @definition.function

; Kanban.
(kanban_item
  id: (kanban_item_id) @name) @definition.var

; Mindmap.
(mindmap_node
  id: (mindmap_node_id) @name) @definition.var

; Radar.
[
  (radar_axis
    name: (radar_identifier) @name)
  (radar_curve
    name: (radar_identifier) @name)
] @definition.var

; Railroad constructor dialect.
(railroad_rule
  name: (railroad_identifier) @name) @definition.function

; Railroad ABNF.
(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @name) @definition.function

; Railroad EBNF.
(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @name) @definition.function

; Railroad PEG.
(railroad_peg_rule
  name: (railroad_peg_identifier) @name) @definition.function

; Requirement.
[
  (requirement_declaration
    name: (requirement_name) @name)
  (requirement_element_declaration
    name: (requirement_name) @name)
] @definition.class

; Sequence.
(sequence_participant_declaration
  name: (sequence_participant_name) @name) @definition.class

; State.
(state_alias_clause
  name: (state_name) @name) @definition.class

[
  (state_named_declaration
    name: (state_name) @name)
  (state_pseudostate_declaration
    name: (state_name) @name)
] @definition.class

(state_composite_declaration
  name: (state_name) @name) @definition.module

; Swimlane.
(swimlane_subgraph
  id: (swimlane_node_id) @name) @definition.module

(swimlane_vertex
  id: (swimlane_node_id) @name
  shape: (_)) @definition.var

; Timeline.
(timeline_section_statement
  name: (timeline_section_name) @name) @definition.module

; Tree View.
(tree_view_node
  name: (_) @name) @definition.var

; Treemap.
[
  (treemap_section
    name: (_) @name)
  (treemap_leaf
    name: (_) @name)
] @definition.var

; Venn.
(venn_set_statement
  expression: (venn_set_expression
    set: (venn_identifier) @name)) @definition.var

; Wardley.
[
  (wardley_component_statement
    name: (wardley_name) @name)
  (wardley_anchor_statement
    name: (wardley_name) @name)
] @definition.var

; ZenUML.
[
  (zenuml_starter_declaration
    participant: (zenuml_name) @name)
  (zenuml_participant_declaration
    name: (zenuml_name) @name)
] @definition.class

(zenuml_assignment
  assignee: (zenuml_assignee
    item: (zenuml_identifier) @name)) @definition.var
