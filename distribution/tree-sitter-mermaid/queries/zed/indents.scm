; Zed auto-indentation profile. @indent is required by Zed; @end aligns
; closing delimiters and family-owned block terminators with their opener.

; Anonymous delimiters are attached to explicit CST owners. This avoids
; double-indenting nodes such as C4 boundaries that contain both a call
; argument list and a block body.
[
  (architecture_icon ")" @end)
  (c4_entity_declaration ")" @end)
  (c4_relationship_statement ")" @end)
  (c4_style_update_statement ")" @end)
  (class_call_action ")" @end)
  (gantt_call_action ")" @end)
  (railroad_sequence ")" @end)
  (railroad_terminal ")" @end)
  (railroad_reference ")" @end)
  (railroad_repetition ")" @end)
  (railroad_special ")" @end)
  (railroad_peg_group ")" @end)
  (wardley_strategy_decorator ")" @end)
  (zenuml_starter_declaration ")" @end)
  (zenuml_argument_list ")" @end)
  (zenuml_condition_clause ")" @end)
] @indent

[
  (architecture_title "]" @end)
  (er_entity_alias "]" @end)
  (quadrant_chart_coordinates "]" @end)
  (radar_label "]" @end)
  (railroad_abnf_optional_group "]" @end)
  (railroad_ebnf_optional_group "]" @end)
  (wardley_position "]" @end)
] @indent

[
  (c4_boundary_statement "}" @end)
  (class_namespace_declaration "}" @end)
  (class_member_block "}" @end)
  (er_attribute_block "}" @end)
  (event_data_block "}" @end)
  (event_nested_data_block "}" @end)
  (radar_curve_entries "}" @end)
  (railroad_ebnf_repetition_group "}" @end)
  (requirement_declaration "}" @end)
  (requirement_element_declaration "}" @end)
  (state_composite_declaration "}" @end)
  (zenuml_block "}" @end)
] @indent

(_
  open: (block_shape_delimiter)
  close: (block_shape_delimiter) @end) @indent

(_
  open: (flow_shape_delimiter)
  close: (flow_shape_delimiter) @end) @indent

(_
  open: (swimlane_shape_delimiter)
  close: (swimlane_shape_delimiter) @end) @indent

(_
  open: (mindmap_shape_delimiter)
  close: (mindmap_shape_delimiter) @end) @indent

(_
  open: (kanban_shape_delimiter)
  close: (kanban_shape_delimiter) @end) @indent

(_
  open: (kanban_metadata_delimiter)
  close: (kanban_metadata_delimiter) @end) @indent

[
  (xy_chart_category_array
    close: (xy_chart_array_close) @end)
  (xy_chart_series_array
    close: (xy_chart_array_close) @end)
] @indent

(block_composite_statement
  end: (block_end) @end) @indent

(flow_subgraph
  end: (flow_subgraph_end) @end) @indent

(swimlane_subgraph
  end: (swimlane_subgraph_end) @end) @indent

[
  (sequence_alt_block
    end: (sequence_block_end) @end)
  (sequence_box_block
    end: (sequence_block_end) @end)
  (sequence_break_block
    end: (sequence_block_end) @end)
  (sequence_critical_block
    end: (sequence_block_end) @end)
  (sequence_loop_block
    end: (sequence_block_end) @end)
  (sequence_opt_block
    end: (sequence_block_end) @end)
  (sequence_par_block
    end: (sequence_block_end) @end)
  (sequence_rect_block
    end: (sequence_block_end) @end)
] @indent

(state_multiline_note
  end: (state_note_end) @end) @indent
