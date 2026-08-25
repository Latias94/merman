; Zed bracket-pair profile. Capture names follow Zed's brackets query ABI.

; Anonymous delimiter pairs used by block, expression, and shape productions.
("(" @open ")" @close)
("[" @open "]" @close)
("{" @open "}" @close)

; Family-owned named delimiters preserve exact source ranges.
(_
  open: (block_shape_delimiter) @open
  close: (block_shape_delimiter) @close)

(_
  open: (flow_shape_delimiter) @open
  close: (flow_shape_delimiter) @close)

(_
  open: (swimlane_shape_delimiter) @open
  close: (swimlane_shape_delimiter) @close)

(_
  open: (mindmap_shape_delimiter) @open
  close: (mindmap_shape_delimiter) @close)

(_
  open: (mindmap_decorator_delimiter) @open
  close: (mindmap_decorator_delimiter) @close)

(_
  open: (kanban_shape_delimiter) @open
  close: (kanban_shape_delimiter) @close)

(_
  open: (kanban_decorator_delimiter) @open
  close: (kanban_decorator_delimiter) @close)

(_
  open: (kanban_metadata_delimiter) @open
  close: (kanban_metadata_delimiter) @close)

(_
  open: (venn_label_delimiter) @open
  close: (venn_label_delimiter) @close)

((sankey_quoted_field
  open: (sankey_quote) @open
  close: (sankey_quote) @close)
  (#set! rainbow.exclude))

((_
  open: (xy_chart_quote_delimiter) @open
  close: (xy_chart_quote_delimiter) @close)
  (#set! rainbow.exclude))

((_
  open: (xy_chart_markdown_delimiter) @open
  close: (xy_chart_markdown_delimiter) @close)
  (#set! rainbow.exclude))

; XY arrays expose dedicated named open and close nodes.
(xy_chart_category_array
  open: (xy_chart_array_open) @open
  close: (xy_chart_array_close) @close)

(xy_chart_series_array
  open: (xy_chart_array_open) @open
  close: (xy_chart_array_close) @close)

; Mermaid's keyword-delimited blocks are bracket pairs in Zed as well.
(block_composite_statement
  keyword: (block_statement_keyword) @open
  end: (block_end) @close)

(flow_subgraph
  keyword: (flow_statement_keyword) @open
  end: (flow_subgraph_end) @close)

(swimlane_subgraph
  keyword: (swimlane_statement_keyword) @open
  end: (swimlane_subgraph_end) @close)

[
  (sequence_alt_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_box_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_break_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_critical_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_loop_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_opt_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_par_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
  (sequence_rect_block
    keyword: (sequence_block_keyword) @open
    end: (sequence_block_end) @close)
]

(state_multiline_note
  keyword: (state_statement_keyword) @open
  end: (state_note_end) @close)
