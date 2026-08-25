; Tree-sitter tags capture vocabulary. Every Mermaid file defines one diagram
; module; declaration-level tags add useful navigation without pretending that
; narrative-only families have local variables.

(source_file
  (_
    header: (_
      keyword: (diagram_keyword) @name)) @definition.module)

(architecture_group_statement
  id: (architecture_identifier) @name) @definition.module

[
  (architecture_service_statement
    id: (architecture_identifier) @name)
  (architecture_junction_statement
    id: (architecture_identifier) @name)
] @definition.var

(block_node
  id: (block_identifier) @name
  shape: (_)) @definition.var

[
  (c4_boundary_statement
    id: (c4_reference
      value: (_) @name))
  (class_namespace_declaration
    name: (class_namespace_name) @name)
] @definition.module

(c4_entity_declaration
  id: (c4_reference
    value: (_) @name)) @definition.var

(class_declaration
  name: (class_name) @name) @definition.class

(er_entity_declaration
  name: (er_entity_name) @name) @definition.class

(event_entity_statement
  name: (event_qualified_name) @name) @definition.class

(event_data_statement
  name: (event_data_name) @name) @definition.var

(flow_vertex
  id: (flow_node_id) @name
  shape: (_)) @definition.var

(gantt_section_statement
  name: (gantt_line_text) @name) @definition.module

(gantt_task_statement
  name: (gantt_task_name) @name) @definition.var

(git_graph_branch_statement
  name: (_) @name) @definition.var

(journey_section_statement
  section: (journey_section_name) @name) @definition.module

(kanban_item
  id: (kanban_item_id) @name) @definition.var

(mindmap_node
  id: (mindmap_node_id) @name) @definition.var

(quadrant_chart_point_statement
  label: (quadrant_chart_point_label) @name) @definition.var

[
  (radar_axis
    name: (radar_identifier) @name)
  (radar_curve
    name: (radar_identifier) @name)
] @definition.var

(railroad_rule
  name: (railroad_identifier) @name) @definition.var

(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @name) @definition.var

(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @name) @definition.var

(railroad_peg_rule
  name: (railroad_peg_identifier) @name) @definition.var

[
  (requirement_declaration
    name: (requirement_name) @name)
  (requirement_element_declaration
    name: (requirement_name) @name)
] @definition.var

(sequence_participant_declaration
  name: (sequence_participant_name) @name) @definition.var

[
  (state_named_declaration
    name: (state_name) @name)
  (state_pseudostate_declaration
    name: (state_name) @name)
] @definition.var

(state_composite_declaration
  name: (state_name) @name) @definition.module

(swimlane_vertex
  id: (swimlane_node_id) @name
  shape: (_)) @definition.var

(timeline_section_statement
  name: (timeline_section_name) @name) @definition.module

(tree_view_node
  name: (_) @name) @definition.var

[
  (treemap_section
    name: (_) @name)
  (treemap_leaf
    name: (_) @name)
] @definition.var

(venn_set_statement
  expression: (venn_set_expression
    set: (venn_identifier) @name)) @definition.var

[
  (wardley_component_statement
    name: (wardley_name) @name)
  (wardley_anchor_statement
    name: (wardley_name) @name)
] @definition.var

[
  (zenuml_starter_declaration
    participant: (zenuml_name) @name)
  (zenuml_participant_declaration
    name: (zenuml_name) @name)
] @definition.var

(zenuml_assignment
  assignee: (zenuml_assignee
    item: (zenuml_identifier) @name)) @definition.var
