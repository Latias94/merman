; Standalone family fragment. Keep the central shared captures once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

(xy_chart_beta_marker) @attribute
(xy_chart_orientation) @constant

[
  (xy_chart_quoted_text)
  (xy_chart_markdown_text)
  (xy_chart_bare_text)
  (xy_chart_accessibility_text)
  (xy_chart_accessibility_block_text)
] @string

(xy_chart_axis_range
  [
    (xy_chart_number)
  ] @number)

(xy_chart_incomplete_axis_range
  [
    (xy_chart_number)
  ] @number)

(xy_chart_data_point
  value: (xy_chart_number) @number)

(xy_chart_range_delimiter) @operator

[
  (xy_chart_array_open)
  (xy_chart_array_close)
] @punctuation.bracket

[
  (xy_chart_array_delimiter)
  (xy_chart_accessibility_delimiter)
  (xy_chart_statement_delimiter)
] @punctuation.delimiter
