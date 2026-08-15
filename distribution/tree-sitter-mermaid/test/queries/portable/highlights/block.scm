; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(block_statement_keyword) @keyword
(block_end) @keyword

(block_identifier) @variable
(block_arrow_direction) @constant

[
  (block_quoted_label)
  (block_bare_label)
  (block_line_text)
  (block_accessibility_description_block)
  (block_unclosed_accessibility_description_block)
  (block_style_value)
] @string

[
  (block_column_count)
  (block_width)
] @number

(block_style_property) @property

[
  (block_edge_label_start)
  (block_edge_operator)
] @operator

(block_shape_delimiter) @punctuation.bracket

(block_space_statement delimiter: ":" @punctuation.delimiter)
(block_width_clause delimiter: ":" @punctuation.delimiter)
(block_identifier_list delimiter: "," @punctuation.delimiter)
(block_style_list delimiter: "," @punctuation.delimiter)
(block_style_declaration delimiter: ":" @punctuation.delimiter)
