(diagram_keyword) @keyword
(railroad_statement_keyword) @keyword
(railroad_line_text) @string
(railroad_constructor_keyword) @keyword

(railroad_rule
  name: (railroad_identifier) @function)

(railroad_assignment_operator) @operator

(railroad_terminal
  value: (railroad_string) @string)

(railroad_reference
  name: (railroad_string) @variable)

(railroad_special
  text: (railroad_string) @string.special)

(railroad_block_comment) @comment
