; nvim-treesitter-textobjects-compatible captures.

(source_file
  (_
    body: (_) @class.inner) @class.outer)

[
  (block_composite_statement)
  (c4_boundary_statement)
  (class_member_block)
  (class_namespace_declaration)
  (cynefin_domain_block)
  (er_attribute_block)
  (event_data_block)
  (event_nested_data_block)
  (flow_subgraph)
  (requirement_declaration)
  (requirement_element_declaration)
  (sequence_alt_block)
  (sequence_and_branch)
  (sequence_box_block)
  (sequence_break_block)
  (sequence_critical_block)
  (sequence_else_branch)
  (sequence_loop_block)
  (sequence_opt_block)
  (sequence_option_branch)
  (sequence_par_block)
  (sequence_rect_block)
  (state_composite_declaration)
  (state_multiline_note)
  (swimlane_subgraph)
  (wardley_pipeline_statement)
  (zenuml_block)
  (zenuml_group_block)
] @block.outer

[
  (c4_boundary_body)
  (class_member)
  (class_namespace_body)
  (event_data_fragment)
  (flow_line_item)
  (sequence_body)
  (state_body)
  (swimlane_line_item)
  (wardley_pipeline_body)
  (zenuml_body)
] @block.inner

[
  (zenuml_if_statement)
  (zenuml_else_if_clause)
  (zenuml_else_clause)
  (zenuml_optional_statement)
] @conditional.outer

[
  (sequence_loop_block)
  (zenuml_loop_statement)
] @loop.outer

[
  (railroad_rule)
  (railroad_abnf_rule)
  (railroad_ebnf_rule)
  (railroad_peg_rule)
  (zenuml_call_expression)
  (zenuml_signature)
] @function.outer

[
  (zenuml_argument)
  (zenuml_named_argument)
] @parameter.inner @parameter.outer

[
  (zenuml_assignment)
  (zenuml_assignment_expression)
] @assignment.outer

[
  (comment)
  (event_line_comment)
  (event_multiline_comment)
  (journey_hash_comment)
  (railroad_abnf_comment)
  (railroad_block_comment)
  (railroad_ebnf_block_comment)
  (railroad_ebnf_iso_comment)
  (railroad_peg_comment)
  (requirement_hash_comment)
  (sequence_hash_comment)
  (state_hash_comment)
  (timeline_hash_comment)
  (zenuml_comment)
] @comment.outer
