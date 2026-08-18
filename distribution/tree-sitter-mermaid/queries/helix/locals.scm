; Helix local scopes are diagram-local: Mermaid identifiers do not cross a
; diagram boundary. Only families with a syntactic definition/reference split
; are included; label-only and record-only families are explicit N/A cells.

[
  (architecture_body)
  (block_body)
  (c4_body)
  (c4_boundary_body)
  (class_body)
  (class_namespace_body)
  (entity_relationship_body)
  (flow_body)
  (flow_subgraph)
  (git_graph_body)
  (requirement_body)
  (sequence_body)
  (state_body)
  (state_composite_declaration)
  (swimlane_body)
  (swimlane_subgraph)
  (venn_body)
  (railroad_body)
  (railroad_abnf_body)
  (railroad_ebnf_body)
  (railroad_peg_body)
  (zenuml_body)
  (zenuml_block)
  (zenuml_group_block)
] @local.scope

; Architecture.
[
  (architecture_group_statement id: (architecture_identifier) @local.definition.namespace)
  (architecture_service_statement id: (architecture_identifier) @local.definition.variable)
  (architecture_junction_statement id: (architecture_identifier) @local.definition.variable)
]

(architecture_parent_clause parent: (architecture_identifier) @local.reference)
(architecture_edge_endpoint id: (architecture_identifier) @local.reference)
(architecture_alignment_statement member: (architecture_identifier) @local.reference)

; Block diagrams.
(block_node_statement
  node: (block_node id: (block_identifier) @local.definition.variable))
(block_composite_statement
  node: (block_node id: (block_identifier) @local.definition.variable))
(block_edge_statement
  source: (block_node id: (block_identifier) @local.reference)
  target: (block_node id: (block_identifier) @local.reference))

; C4.
(c4_boundary_statement id: (c4_reference) @local.definition.namespace)
(c4_entity_declaration id: (c4_reference) @local.definition.variable)
(c4_relationship_statement
  source: (c4_reference) @local.reference
  target: (c4_reference) @local.reference)
(c4_style_update_statement
  source: (c4_reference)? @local.reference
  target: (c4_reference)? @local.reference)

; Class and entity-relationship diagrams.
(class_namespace_declaration name: (class_namespace_name) @local.definition.namespace)
(class_declaration name: (class_name) @local.definition.type)
(class_reference) @local.reference

(er_entity_declaration name: (er_entity_name) @local.definition.type)
(er_relationship
  source: (er_entity_reference) @local.reference
  target: (er_entity_reference) @local.reference)

; Flowchart and swimlane explicit definitions plus references.
(flow_subgraph id: (flow_node_id) @local.definition.namespace)
(flow_node_statement
  node: (flow_node
    vertex: (flow_vertex id: (flow_node_id) @local.definition.variable)))
(flow_edge_statement
  source: (flow_node (flow_vertex id: (flow_node_id) @local.reference))
  target: (flow_node (flow_vertex id: (flow_node_id) @local.reference)))
(flow_reference) @local.reference

(swimlane_subgraph id: (swimlane_node_id) @local.definition.namespace)
(swimlane_node_statement
  node: (swimlane_node
    vertex: (swimlane_vertex id: (swimlane_node_id) @local.definition.variable)))
(swimlane_edge_statement
  source: (swimlane_node (swimlane_vertex id: (swimlane_node_id) @local.reference))
  target: (swimlane_node (swimlane_vertex id: (swimlane_node_id) @local.reference)))
(swimlane_reference) @local.reference

; GitGraph.
(git_graph_branch_statement name: (_) @local.definition.variable)
(git_graph_checkout_statement branch: (_) @local.reference)
(git_graph_merge_statement branch: (_) @local.reference)

; Requirements.
(requirement_declaration name: (requirement_name) @local.definition.variable)
(requirement_element_declaration name: (requirement_name) @local.definition.variable)
(requirement_relationship_statement
  source: (requirement_reference) @local.reference
  target: (requirement_reference) @local.reference)

; Sequence and state diagrams.
(sequence_participant_declaration
  name: (sequence_participant_name) @local.definition.type)
(sequence_actor_reference) @local.reference

(state_named_declaration name: (state_name) @local.definition.variable)
(state_composite_declaration name: (state_name) @local.definition.namespace)
(state_alias_declaration
  alias: (state_alias_clause name: (state_name) @local.definition.variable))
(state_reference) @local.reference

; Venn sets.
(venn_set_statement
  expression: (venn_set_expression set: (venn_identifier) @local.definition.variable))
(venn_intersection_expression set: (venn_identifier) @local.reference)

; Railroad dialect rule definitions and references.
(railroad_rule name: (railroad_identifier) @local.definition.function)
(railroad_reference name: (_) @local.reference)
(railroad_abnf_rule name: (railroad_abnf_rule_name) @local.definition.function)
(railroad_abnf_reference name: (railroad_abnf_rule_name) @local.reference)
(railroad_ebnf_rule name: (railroad_ebnf_identifier) @local.definition.function)
(railroad_ebnf_reference name: (railroad_ebnf_identifier) @local.reference)
(railroad_peg_rule name: (railroad_peg_identifier) @local.definition.function)
(railroad_peg_reference name: (railroad_peg_identifier) @local.reference)

; ZenUML participants and groups.
(zenuml_participant_declaration name: (zenuml_name) @local.definition.type)
(zenuml_starter_declaration participant: (zenuml_name) @local.definition.type)
(zenuml_group_declaration name: (zenuml_name) @local.definition.namespace)
(zenuml_endpoint name: (zenuml_name) @local.reference)
(zenuml_reference_list participant: (zenuml_name) @local.reference)
