; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

(venn_set_expression set: (venn_identifier) @variable)
(venn_intersection_expression set: (venn_identifier) @variable)
(venn_expression (venn_identifier) @variable)
(venn_text_value (venn_identifier) @string)

(venn_title_text) @string
(venn_label
  text: [
    (venn_quoted_label)
    (venn_unquoted_label)
  ] @string)

(venn_number) @number
(venn_color) @string.special
(venn_style_property) @property
(venn_style_atom) @constant

(venn_quote) @punctuation.bracket
(venn_label_delimiter) @punctuation.bracket
(venn_set_delimiter) @punctuation.delimiter
(venn_value_delimiter) @punctuation.delimiter
(venn_style_delimiter) @punctuation.delimiter
