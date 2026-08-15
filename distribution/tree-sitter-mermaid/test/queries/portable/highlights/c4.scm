; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(c4_statement_keyword) @keyword
(c4_entity_kind) @type
(c4_boundary_kind) @type
(c4_relationship_kind) @keyword.operator
(c4_update_kind) @function.macro
(c4_direction) @constant

(c4_identifier) @variable
(c4_property_name) @property

[
  (c4_string)
  (c4_unclosed_string)
  (c4_unquoted_argument)
  (c4_line_text)
  (c4_accessibility_description_block)
  (c4_unclosed_accessibility_description_block)
] @string

(c4_named_argument sigil: "$" @punctuation.special)
(c4_named_argument operator: "=" @operator)

[
  (c4_entity_declaration open: "(")
  (c4_entity_declaration close: ")")
  (c4_boundary_statement open: "{")
  (c4_boundary_statement close: "}")
  (c4_relationship_statement open: "(")
  (c4_relationship_statement close: ")")
  (c4_style_update_statement open: "(")
  (c4_style_update_statement close: ")")
] @punctuation.bracket

[
  (c4_entity_declaration delimiter: ",")
  (c4_boundary_statement delimiter: ",")
  (c4_relationship_statement delimiter: ",")
  (c4_style_update_statement delimiter: ",")
] @punctuation.delimiter
