; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword

(kanban_item
  id: (kanban_item_id) @variable)

[
  (kanban_plain_label)
  (kanban_label_text)
  (kanban_quoted_string)
  (kanban_markdown_string)
  (kanban_multiline_label_text)
] @string

(kanban_metadata_pair
  key: (kanban_metadata_key) @property)

(kanban_metadata_bare_value) @string

(kanban_icon_marker) @function.macro
(kanban_icon_name) @string.special
(kanban_class_marker) @punctuation.special
(kanban_class_list) @type

(kanban_shape_delimiter) @punctuation.bracket
(kanban_metadata_delimiter) @punctuation.bracket
(kanban_metadata_separator) @punctuation.delimiter
