; This fragment is standalone for its family golden. When merging it into
; queries/portable/highlights.scm, retain the central diagram_keyword capture
; only once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

(architecture_group_statement
  id: (architecture_identifier) @namespace)

(architecture_service_statement
  id: (architecture_identifier) @variable)

(architecture_junction_statement
  id: (architecture_identifier) @variable)

(architecture_parent_clause
  parent: (architecture_identifier) @namespace)

(architecture_edge_endpoint
  id: (architecture_identifier) @variable)

(architecture_alignment_statement
  member: (architecture_identifier) @variable)

[
  (architecture_alignment_direction)
  (architecture_port_direction)
] @constant

[
  (architecture_arrowhead)
  (architecture_group_modifier)
  (architecture_plain_connector)
] @operator

(architecture_titled_connector
  "-" @operator)

(architecture_left_port
  ":" @punctuation.delimiter)

(architecture_right_port
  ":" @punctuation.delimiter)

(architecture_icon
  "(" @punctuation.delimiter
  ")" @punctuation.delimiter)

(architecture_title
  "[" @punctuation.delimiter
  "]" @punctuation.delimiter)

[
  (architecture_quoted_string)
  (architecture_unclosed_quoted_string)
  (architecture_bare_title)
  (architecture_line_text)
  (architecture_accessibility_text)
] @string

(architecture_icon_name) @string.special
