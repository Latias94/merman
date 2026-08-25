; rainbow-delimiters.nvim-compatible captures. Field-based pairing lets the
; grammar define the actual delimiter tokens, including named aliases.

(_
  open: _ @delimiter
  close: _ @delimiter) @container

(architecture_icon
  "(" @delimiter
  ")" @delimiter) @container

(architecture_title
  "[" @delimiter
  "]" @delimiter) @container

(wardley_position
  "[" @delimiter
  "]" @delimiter) @container

(wardley_xy_position
  "[" @delimiter
  "]" @delimiter) @container

(block_composite_statement
  keyword: (block_statement_keyword) @delimiter
  end: (block_end) @delimiter) @container

(flow_subgraph
  keyword: (flow_statement_keyword) @delimiter
  end: (flow_subgraph_end) @delimiter) @container

(swimlane_subgraph
  keyword: (swimlane_statement_keyword) @delimiter
  end: (swimlane_subgraph_end) @delimiter) @container

[
  (sequence_alt_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_box_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_break_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_critical_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_loop_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_opt_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_par_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
  (sequence_rect_block
    keyword: (sequence_block_keyword) @delimiter
    end: (sequence_block_end) @delimiter)
] @container

(state_multiline_note
  keyword: (state_statement_keyword) @delimiter
  end: (state_note_end) @delimiter) @container
