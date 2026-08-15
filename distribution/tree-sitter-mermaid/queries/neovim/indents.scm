; Neovim indentation captures are emitted only where the CST exposes a
; structural begin/end, branch, or explicit indentation transition.

[
  (block_composite_statement)
  (c4_boundary_statement)
  (class_member_block)
  (class_namespace_declaration)
  (er_entity_declaration
    attributes: (er_attribute_block))
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
  (state_multiline_note)
  (swimlane_subgraph)
  (zenuml_block)
  (zenuml_group_block)
] @indent.begin

[
  (sequence_and_branch)
  (sequence_else_branch)
  (sequence_option_branch)
  (zenuml_catch_clause)
  (zenuml_else_clause)
  (zenuml_else_if_clause)
  (zenuml_finally_clause)
] @indent.branch

[
  (block_end)
  (flow_subgraph_end)
  (sequence_block_end)
  (state_note_end)
  (swimlane_subgraph_end)
] @indent.branch @indent.end

(c4_boundary_statement
  close: "}" @indent.branch @indent.end)

(class_member_block
  close: "}" @indent.branch @indent.end)

(class_namespace_declaration
  close: "}" @indent.branch @indent.end)

(er_attribute_block
  close: "}" @indent.branch @indent.end)

(event_data_block
  close: "}" @indent.branch @indent.end)

(event_nested_data_block
  close: "}" @indent.branch @indent.end)

(requirement_declaration
  close: "}" @indent.branch @indent.end)

(requirement_element_declaration
  close: "}" @indent.branch @indent.end)

(state_composite_declaration
  close: "}" @indent.branch @indent.end)

(zenuml_block
  close: "}" @indent.branch @indent.end)

(zenuml_group_block
  close: "}" @indent.branch @indent.end)

(tree_view_indentation_indent) @indent.begin
(tree_view_indentation_reindent) @indent.branch
(tree_view_indentation_dedent) @indent.branch @indent.end
(tree_view_indentation_overflow) @indent.auto

(treemap_indentation_indent) @indent.begin
(treemap_indentation_reindent) @indent.branch
(treemap_indentation_dedent) @indent.branch @indent.end
(treemap_indentation_overflow) @indent.auto

(ishikawa_indentation) @indent.auto

[
  (comment)
  (directive)
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
] @indent.ignore
