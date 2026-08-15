; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(swimlane_statement_keyword) @keyword
(swimlane_subgraph_end) @keyword

[
  (swimlane_node_id)
  (swimlane_reference)
] @variable

(swimlane_edge_name) @variable.member
(swimlane_class_name) @type
(swimlane_callback_name) @function

[
  (swimlane_quoted_label)
  (swimlane_markdown_label)
  (swimlane_label_text)
  (swimlane_square_label_text)
  (swimlane_round_label_text)
  (swimlane_curly_label_text)
  (swimlane_edge_label_text)
  (swimlane_middle_edge_label_text)
  (swimlane_shape_data_string)
  (swimlane_style_value)
  (swimlane_accessibility_text)
  (swimlane_accessibility_block_text)
] @string

[
  (direction)
  (swimlane_direction)
  (swimlane_link_target)
] @constant

(swimlane_style_property) @property
(swimlane_edge_index) @number

[
  (swimlane_arrow)
  (swimlane_arrow_start)
  (swimlane_continued_arrow)
  (swimlane_continued_arrow_start)
] @operator

(swimlane_shape_delimiter) @punctuation.bracket

(swimlane_edge_id delimiter: "@" @punctuation.delimiter)
(swimlane_edge_label open: "|" @punctuation.delimiter)
(swimlane_edge_label close: "|" @punctuation.delimiter)
(swimlane_identifier_list delimiter: "," @punctuation.delimiter)
(swimlane_number_list delimiter: "," @punctuation.delimiter)
(swimlane_style_list delimiter: "," @punctuation.delimiter)
(swimlane_style_declaration delimiter: ":" @punctuation.delimiter)
