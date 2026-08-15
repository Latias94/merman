; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(flow_statement_keyword) @keyword
(flow_subgraph_end) @keyword

[
  (flow_node_id)
  (flow_reference)
] @variable

(flow_edge_name) @variable.member
(flow_class_name) @type
(flow_callback_name) @function

[
  (flow_quoted_label)
  (flow_markdown_label)
  (flow_label_text)
  (flow_square_label_text)
  (flow_round_label_text)
  (flow_curly_label_text)
  (flow_edge_label_text)
  (flow_middle_edge_label_text)
  (flow_shape_data_string)
  (flow_style_value)
  (flow_accessibility_text)
  (flow_accessibility_block_text)
] @string

[
  (direction)
  (flow_direction)
  (flow_link_target)
] @constant

(flow_style_property) @property
(flow_edge_index) @number

[
  (flow_arrow)
  (flow_arrow_start)
  (flow_continued_arrow)
  (flow_continued_arrow_start)
] @operator

(flow_shape_delimiter) @punctuation.bracket

(flow_edge_id delimiter: "@" @punctuation.delimiter)
(flow_edge_label open: "|" @punctuation.delimiter)
(flow_edge_label close: "|" @punctuation.delimiter)
(flow_identifier_list delimiter: "," @punctuation.delimiter)
(flow_number_list delimiter: "," @punctuation.delimiter)
(flow_style_list delimiter: "," @punctuation.delimiter)
(flow_style_declaration delimiter: ":" @punctuation.delimiter)
