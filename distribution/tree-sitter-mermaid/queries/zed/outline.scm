; Zed outline profile. Every family contributes its diagram root, while
; declaration-bearing families expose stable nested items when the CST has a
; source-owned name field.

(_
  header: (_
    (diagram_keyword) @name)) @item

(architecture_group_statement
  id: (architecture_identifier) @name) @item

[
  (architecture_service_statement
    id: (architecture_identifier) @name)
  (architecture_junction_statement
    id: (architecture_identifier) @name)
] @item

(block_node_statement
  node: (block_node
    id: (block_identifier) @name)) @item

[
  (c4_entity_declaration
    id: (c4_reference) @name)
  (c4_boundary_statement
    id: (c4_reference) @name)
] @item

(class_definition_statement
  name: (class_style_name) @name) @item

(er_entity_declaration
  name: (er_entity_name) @name) @item

[
  (event_entity_statement
    name: (event_qualified_name) @name)
  (event_data_statement
    name: (event_data_name) @name)
  (event_frame_statement
    frame: (event_frame_id) @name)
] @item

(flow_vertex
  id: (flow_node_id) @name) @item

(flow_subgraph
  id: (flow_node_id) @name) @item

[
  (gantt_section_statement
    name: (gantt_line_text) @name)
  (gantt_task_statement
    name: (gantt_task_name) @name)
] @item

(git_graph_branch_statement
  name: (_) @name) @item

[
  (ishikawa_effect_statement
    label: (ishikawa_label) @name)
  (ishikawa_cause_statement
    label: (ishikawa_label) @name)
] @item

[
  (journey_section_statement
    section: (journey_section_name) @name)
  (journey_task_statement
    task: (journey_task_name) @name)
] @item

(kanban_item_statement
  item: (kanban_item
    [
      id: (kanban_item_id) @name
      label: (kanban_plain_label) @name
    ])) @item

(mindmap_node_statement
  node: (mindmap_node
    [
      id: (mindmap_node_id) @name
      label: (mindmap_plain_label) @name
    ])) @item

(packet_block_statement
  label: (_) @name) @item

(pie_section
  label: (_) @name) @item

(quadrant_chart_point_statement
  label: (quadrant_chart_point_label) @name) @item

[
  (radar_axis
    name: (radar_identifier) @name)
  (radar_curve
    name: (radar_identifier) @name)
] @item

[
  (railroad_rule
    name: (railroad_identifier) @name)
  (railroad_abnf_rule
    name: (railroad_abnf_rule_name) @name)
  (railroad_ebnf_rule
    name: (railroad_ebnf_identifier) @name)
  (railroad_peg_rule
    name: (railroad_peg_identifier) @name)
] @item

[
  (requirement_declaration
    name: (requirement_name) @name)
  (requirement_element_declaration
    name: (requirement_name) @name)
] @item

(sequence_participant_declaration
  name: (sequence_participant_name) @name) @item

[
  (state_alias_declaration
    alias: (state_alias_clause
      name: (state_name) @name))
  (state_named_declaration
    name: (state_name) @name)
  (state_composite_declaration
    name: (state_name) @name)
  (state_pseudostate_declaration
    name: (state_name) @name)
] @item

(swimlane_vertex
  id: (swimlane_node_id) @name) @item

(swimlane_subgraph
  id: (swimlane_node_id) @name) @item

(timeline_section_statement
  name: (timeline_section_name) @name) @item

(tree_view_node
  name: (_) @name) @item

(treemap_section
  name: (_) @name) @item

(venn_set_statement
  expression: (venn_set_expression) @name) @item

(wardley_component_statement
  name: (wardley_name) @name) @item

[
  (xy_chart_bar_statement
    title: (xy_chart_text) @name)
  (xy_chart_line_statement
    title: (xy_chart_text) @name)
] @item

(zenuml_participant_declaration
  name: (zenuml_name) @name) @item
