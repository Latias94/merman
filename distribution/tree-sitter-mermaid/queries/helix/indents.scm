; Helix indentation is only claimed for CST containers whose source range owns
; both an opener and a closer. Flat/indentation-scanner families are explicit
; N/A until their CST exposes nested parent ranges Helix can traverse.

[
  (block_composite_statement)
  (c4_boundary_statement)
  (class_namespace_declaration)
  (class_member_block)
  (event_data_block)
  (event_nested_data_block)
  (flow_subgraph)
  (requirement_declaration)
  (requirement_element_declaration)
  (sequence_alt_block)
  (sequence_box_block)
  (sequence_break_block)
  (sequence_critical_block)
  (sequence_loop_block)
  (sequence_opt_block)
  (sequence_par_block)
  (sequence_rect_block)
  (state_composite_declaration)
  (swimlane_subgraph)
  (zenuml_block)
  (zenuml_group_block)
] @indent

[
  (block_end)
  (flow_subgraph_end)
  (sequence_block_end)
  (swimlane_subgraph_end)
] @outdent

(c4_boundary_statement close: "}" @outdent)
(class_namespace_declaration close: "}" @outdent)
(class_member_block close: "}" @outdent)
(event_data_block close: "}" @outdent)
(event_nested_data_block close: "}" @outdent)
(requirement_declaration close: "}" @outdent)
(requirement_element_declaration close: "}" @outdent)
(state_composite_declaration close: "}" @outdent)
(zenuml_block close: "}" @outdent)
(zenuml_group_block close: "}" @outdent)
