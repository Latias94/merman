; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(requirement_statement_keyword) @keyword
(requirement_attribute_keyword) @property
(requirement_kind) @type
(requirement_relationship_kind) @keyword.operator

[
  (requirement_direction)
  (requirement_risk)
  (requirement_verify_method)
] @constant

[
  (requirement_unquoted_name)
  (requirement_unquoted_reference)
  (requirement_style_identifier)
] @variable

[
  (requirement_string)
  (requirement_unclosed_string)
  (requirement_attribute_text)
  (requirement_line_text)
  (requirement_accessibility_block_text)
  (requirement_style_value)
] @string

(requirement_style_property) @property
(requirement_relationship_operator) @operator
(requirement_hash_comment) @comment

(requirement_attribute delimiter: ":" @punctuation.delimiter)
(requirement_element_attribute delimiter: ":" @punctuation.delimiter)
(requirement_class_annotation delimiter: ":::" @punctuation.delimiter)
(requirement_class_annotation delimiter: "," @punctuation.delimiter)
(requirement_identifier_list delimiter: "," @punctuation.delimiter)
(requirement_style_declaration delimiter: ":" @punctuation.delimiter)

(requirement_declaration open: "{" @punctuation.bracket)
(requirement_declaration close: "}" @punctuation.bracket)
(requirement_element_declaration open: "{" @punctuation.bracket)
(requirement_element_declaration close: "}" @punctuation.bracket)
