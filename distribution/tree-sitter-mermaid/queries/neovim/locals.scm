; Diagram-local symbols. Families without a coherent definition/reference
; relation are explicitly N/A in the Neovim applicability evidence.

[
  (architecture_body)
  (block_body)
  (c4_body)
  (class_body)
  (entity_relationship_body)
  (event_modeling_body)
  (flow_body)
  (gantt_body)
  (git_graph_body)
  (radar_body)
  (railroad_body)
  (railroad_abnf_body)
  (railroad_ebnf_body)
  (railroad_peg_body)
  (requirement_body)
  (sequence_body)
  (state_body)
  (swimlane_body)
  (venn_body)
  (wardley_body)
  (zenuml_body)
  (class_namespace_body)
  (state_composite_declaration)
  (zenuml_block)
  (zenuml_group_block)
] @local.scope

; Architecture.
(architecture_group_statement
  id: (architecture_identifier) @local.definition.namespace)

[
  (architecture_service_statement
    id: (architecture_identifier) @local.definition.var)
  (architecture_junction_statement
    id: (architecture_identifier) @local.definition.var)
]

(architecture_parent_clause
  parent: (architecture_identifier) @local.reference)

(architecture_edge_endpoint
  id: (architecture_identifier) @local.reference)

(architecture_alignment_statement
  member: (architecture_identifier) @local.reference)

; Block.
(block_node
  id: (block_identifier) @local.definition.var
  shape: (_))

(block_edge_statement
  source: (block_node
    id: (block_identifier) @local.reference
    !shape))

(block_edge_statement
  target: (block_node
    id: (block_identifier) @local.reference
    !shape))

; C4.
[
  (c4_boundary_statement
    id: (c4_reference
      value: (_) @local.definition.namespace))
  (c4_entity_declaration
    id: (c4_reference
      value: (_) @local.definition.var))
]

(c4_relationship_statement
  source: (c4_reference
    value: (_) @local.reference))

(c4_relationship_statement
  target: (c4_reference
    value: (_) @local.reference))

(c4_style_update_statement
  target: (c4_reference
    value: (_) @local.reference))

; Class and ER.
(class_namespace_declaration
  name: (class_namespace_name) @local.definition.namespace)

(class_declaration
  name: (class_name) @local.definition.type)

(class_reference
  (identifier) @local.reference)

(er_entity_declaration
  name: (er_entity_name) @local.definition.type)

(er_relationship
  source: (er_entity_reference) @local.reference)

(er_relationship
  target: (er_entity_reference) @local.reference)

; Event Modeling.
(event_entity_statement
  name: (event_qualified_name) @local.definition.type)

(event_data_statement
  name: (event_data_name) @local.definition.var)

(event_frame_statement
  entity: (event_qualified_name) @local.reference)

(event_frame_statement
  data_reference: (event_data_reference
    name: (event_data_name) @local.reference))

; Flowchart and Swimlane.
(flow_vertex
  id: (flow_node_id) @local.definition.var
  shape: (_))

(flow_vertex
  id: (flow_node_id) @local.reference
  !shape)

[
  (flow_style_statement
    target: (flow_node_id) @local.reference)
  (flow_click_statement
    target: (flow_node_id) @local.reference)
  (flow_reference) @local.reference
]

(swimlane_vertex
  id: (swimlane_node_id) @local.definition.var
  shape: (_))

(swimlane_vertex
  id: (swimlane_node_id) @local.reference
  !shape)

[
  (swimlane_style_statement
    target: (swimlane_node_id) @local.reference)
  (swimlane_click_statement
    target: (swimlane_node_id) @local.reference)
  (swimlane_reference) @local.reference
]

; Gantt and GitGraph.
(gantt_task_statement
  name: (gantt_task_name) @local.definition.var)

(gantt_reference) @local.reference

(git_graph_branch_statement
  name: (_) @local.definition.var)

[
  (git_graph_checkout_statement
    branch: (_) @local.reference)
  (git_graph_merge_statement
    branch: (_) @local.reference)
]

; Radar.
(radar_axis
  name: (radar_identifier) @local.definition.var)

(radar_curve
  name: (radar_identifier) @local.definition.var)

(radar_detailed_entry
  axis: (radar_identifier) @local.reference)

; Railroad dialects.
(railroad_rule
  name: (railroad_identifier) @local.definition.var)

(railroad_reference
  name: (_) @local.reference)

(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @local.definition.var)

(railroad_abnf_reference
  name: (railroad_abnf_rule_name) @local.reference)

(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @local.definition.var)

(railroad_ebnf_reference
  name: (railroad_ebnf_identifier) @local.reference)

(railroad_peg_rule
  name: (railroad_peg_identifier) @local.definition.var)

(railroad_peg_reference
  name: (railroad_peg_identifier) @local.reference)

; Requirement.
[
  (requirement_declaration
    name: (requirement_name) @local.definition.var)
  (requirement_element_declaration
    name: (requirement_name) @local.definition.var)
]

(requirement_relationship_statement
  source: (requirement_reference) @local.reference)

(requirement_relationship_statement
  target: (requirement_reference) @local.reference)

; Sequence and State.
(sequence_participant_declaration
  name: (sequence_participant_name) @local.definition.var)

(sequence_actor_reference) @local.reference

(state_alias_clause
  name: (state_name) @local.definition.var)

[
  (state_named_declaration
    name: (state_name) @local.definition.var)
  (state_pseudostate_declaration
    name: (state_name) @local.definition.var)
  (state_composite_declaration
    name: (state_name) @local.definition.namespace)
]

(state_reference) @local.reference

; Venn and Wardley.
(venn_set_statement
  expression: (venn_set_expression
    set: (venn_identifier) @local.definition.var))

(venn_intersection_expression
  set: (venn_identifier) @local.reference)

[
  (wardley_component_statement
    name: (wardley_name) @local.definition.var)
  (wardley_anchor_statement
    name: (wardley_name) @local.definition.var)
]

(wardley_link_statement
  source: (wardley_name) @local.reference)

(wardley_link_statement
  target: (wardley_name) @local.reference)

(wardley_evolve_statement
  component: (wardley_name) @local.reference)

; ZenUML.
[
  (zenuml_starter_declaration
    participant: (zenuml_name) @local.definition.var)
  (zenuml_participant_declaration
    name: (zenuml_name) @local.definition.var)
]

(zenuml_assignment
  assignee: (zenuml_assignee
    item: (zenuml_identifier) @local.definition.var))

(zenuml_endpoint
  name: (zenuml_name) @local.reference)
