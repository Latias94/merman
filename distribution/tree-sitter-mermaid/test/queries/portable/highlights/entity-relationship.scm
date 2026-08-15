; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(er_statement_keyword) @keyword

[
  (er_entity_name)
  (er_entity_reference)
] @type

(er_attribute_type) @type.builtin
(er_attribute_name) @property
(er_attribute_key) @attribute
(er_direction) @constant

[
  (er_cardinality)
  (er_relationship_operator)
] @operator

[
  (er_quoted_text)
  (er_unclosed_quoted_text)
  (er_role_text)
  (er_line_text)
  (er_accessibility_description_block)
  (er_unclosed_accessibility_description_block)
] @string

(er_style_name) @attribute
(er_style_item) @property
