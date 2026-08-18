; Mermaid identifiers are diagram-local. Only families whose CST distinguishes
; declarations from references participate in the portable locals contract.
[
  (architecture_diagram)
  (block_diagram)
  (c4_diagram)
  (class_diagram)
  (entity_relationship_diagram)
  (event_modeling_diagram)
  (flowchart_diagram)
  (gantt_diagram)
  (git_graph_diagram)
  (radar_diagram)
  (railroad_diagram)
  (railroad_abnf_diagram)
  (railroad_ebnf_diagram)
  (railroad_peg_diagram)
  (requirement_diagram)
  (sequence_diagram)
  (state_diagram)
  (swimlane_diagram)
  (venn_diagram)
  (wardley_diagram)
  (zenuml_diagram)
] @local.scope

; Architecture.
(architecture_group_statement
  id: (architecture_identifier) @local.definition)

[
  (architecture_service_statement
    id: (architecture_identifier) @local.definition)
  (architecture_junction_statement
    id: (architecture_identifier) @local.definition)
]

(architecture_parent_clause
  parent: (architecture_identifier) @local.reference)

(architecture_edge_endpoint
  id: (architecture_identifier) @local.reference)

(architecture_alignment_statement
  member: (architecture_identifier) @local.reference)

; Block.
(block_node
  id: (block_identifier) @local.definition
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
(c4_entity_declaration
  id: (c4_reference
    value: (c4_identifier) @local.definition))

(c4_boundary_statement
  id: (c4_reference
    value: (c4_identifier) @local.definition))

(c4_relationship_statement
  source: (c4_reference
    value: (c4_identifier) @local.reference))

(c4_relationship_statement
  target: (c4_reference
    value: (c4_identifier) @local.reference))

(c4_style_update_statement
  source: (c4_reference
    value: (c4_identifier) @local.reference))

(c4_style_update_statement
  target: (c4_reference
    value: (c4_identifier) @local.reference))

; Class.
(class_namespace_declaration
  name: (class_namespace_name
    (identifier) @local.definition))

(class_declaration
  name: (class_name
    (identifier) @local.definition))

(class_reference
  (identifier) @local.reference)

; Entity Relationship.
(er_entity_declaration
  name: (er_entity_name) @local.definition)

(er_relationship
  source: (er_entity_reference) @local.reference)

(er_relationship
  target: (er_entity_reference) @local.reference)

; Event Modeling.
(event_entity_statement
  name: (event_qualified_name) @local.definition)

(event_data_statement
  name: (event_data_name) @local.definition)

(event_frame_statement
  entity: (event_qualified_name) @local.reference)

(event_frame_statement
  data_reference: (event_data_reference
    name: (event_data_name) @local.reference))

; Flowchart.
(flow_vertex
  id: (flow_node_id) @local.definition
  shape: (_))

(flow_vertex
  id: (flow_node_id) @local.reference
  !shape)

(flow_class_assignment_statement
  targets: (flow_identifier_list
    item: (flow_reference) @local.reference))

(flow_style_statement
  target: (flow_node_id) @local.reference)

(flow_click_statement
  target: (flow_node_id) @local.reference)

; Gantt.
(gantt_task_statement
  metadata: (gantt_task_metadata
    (gantt_task_item
      value: (gantt_task_atom) @local.definition)))

(gantt_reference) @local.reference

; GitGraph.
(git_graph_branch_statement
  name: (git_graph_reference) @local.definition)

(git_graph_checkout_statement
  branch: (git_graph_reference) @local.reference)

(git_graph_merge_statement
  branch: (git_graph_reference) @local.reference)

; Radar.
(radar_axis
  name: (radar_identifier) @local.definition)

(radar_curve
  name: (radar_identifier) @local.definition)

(radar_detailed_entry
  axis: (radar_identifier) @local.reference)

; Railroad constructor dialect.
(railroad_rule
  name: (railroad_identifier) @local.definition)

(railroad_reference
  name: (railroad_string) @local.reference)

; Railroad ABNF.
(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @local.definition)

(railroad_abnf_reference
  name: (railroad_abnf_rule_name) @local.reference)

; Railroad EBNF.
(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @local.definition)

(railroad_ebnf_reference
  name: (railroad_ebnf_identifier) @local.reference)

; Railroad PEG.
(railroad_peg_rule
  name: (railroad_peg_identifier) @local.definition)

(railroad_peg_reference
  name: (railroad_peg_identifier) @local.reference)

; Requirement.
[
  (requirement_declaration
    name: (requirement_name) @local.definition)
  (requirement_element_declaration
    name: (requirement_name) @local.definition)
]

(requirement_relationship_statement
  source: (requirement_reference) @local.reference)

(requirement_relationship_statement
  target: (requirement_reference) @local.reference)

; Sequence.
(sequence_participant_declaration
  name: (sequence_participant_name) @local.definition)

(sequence_actor_reference) @local.reference

; State.
(state_alias_clause
  name: (state_name) @local.definition)

[
  (state_named_declaration
    name: (state_name) @local.definition)
  (state_pseudostate_declaration
    name: (state_name) @local.definition)
  (state_composite_declaration
    name: (state_name) @local.definition)
]

(state_reference) @local.reference

; Swimlane.
(swimlane_vertex
  id: (swimlane_node_id) @local.definition
  shape: (_))

(swimlane_vertex
  id: (swimlane_node_id) @local.reference
  !shape)

(swimlane_class_assignment_statement
  targets: (swimlane_identifier_list
    item: (swimlane_reference) @local.reference))

(swimlane_style_statement
  target: (swimlane_node_id) @local.reference)

(swimlane_click_statement
  target: (swimlane_node_id) @local.reference)

; Venn.
(venn_set_statement
  expression: (venn_set_expression
    set: (venn_identifier) @local.definition))

(venn_intersection_expression
  set: (venn_identifier) @local.reference)

; Wardley.
[
  (wardley_component_statement
    name: (wardley_name) @local.definition)
  (wardley_anchor_statement
    name: (wardley_name) @local.definition)
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
    participant: (zenuml_name) @local.definition)
  (zenuml_participant_declaration
    name: (zenuml_name) @local.definition)
]

(zenuml_assignment
  assignee: (zenuml_assignee
    item: (zenuml_identifier) @local.definition))

(zenuml_endpoint
  name: (zenuml_name) @local.reference)
