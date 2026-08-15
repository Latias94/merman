; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

[
  (quadrant_chart_axis)
  (quadrant_chart_quadrant)
] @keyword

(quadrant_chart_axis_delimiter) @operator

[
  (quadrant_chart_line_text)
  (quadrant_chart_accessibility_line_text)
  (quadrant_chart_accessibility_description_block)
  (quadrant_chart_unclosed_accessibility_description_block)
  (quadrant_chart_axis_text)
  (quadrant_chart_label)
  (quadrant_chart_point_label)
  (quadrant_chart_style_value)
] @string

(quadrant_chart_class_name) @type
(quadrant_chart_style_name) @property
(quadrant_chart_coordinate) @number
(quadrant_chart_invalid_coordinate) @number

[
  (quadrant_chart_point_delimiter)
  (quadrant_chart_class_delimiter)
] @punctuation.delimiter

(quadrant_chart_coordinates
  ["[" "]"] @punctuation.bracket
  "," @punctuation.delimiter)

(quadrant_chart_style
  ":" @punctuation.delimiter)

(quadrant_chart_style_list
  "," @punctuation.delimiter)
