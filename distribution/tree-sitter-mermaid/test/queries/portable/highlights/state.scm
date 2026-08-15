; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(state_statement_keyword) @keyword
(state_note_end) @keyword

[
  (state_name)
  (state_reference)
] @variable

(state_class_name) @type

[
  (state_quoted_text)
  (state_description_text)
  (state_note_text)
  (state_note_line)
  (state_style_value)
  (state_accessibility_text)
  (state_accessibility_block_text)
] @string

[
  (state_pseudostate_kind)
  (state_marker)
  (state_direction)
  (state_note_position)
] @constant

(state_style_property) @property
(state_scale_width) @number

[
  (state_transition_operator)
  (state_concurrent_divider)
] @operator

(state_class_annotation operator: ":::" @operator)

(state_transition_statement delimiter: ":" @punctuation.delimiter)
(state_description_statement delimiter: ":" @punctuation.delimiter)
(state_inline_note delimiter: ":" @punctuation.delimiter)
(state_alias_declaration delimiter: ":" @punctuation.delimiter)
(state_identifier_list delimiter: "," @punctuation.delimiter)
(state_style_list delimiter: "," @punctuation.delimiter)
(state_style_declaration delimiter: ":" @punctuation.delimiter)
