(diagram_keyword) @keyword
(railroad_statement_keyword) @keyword
(railroad_line_text) @string

(railroad_peg_rule
  name: (railroad_peg_identifier) @function)

(railroad_peg_reference
  name: (railroad_peg_identifier) @variable)

[
  (railroad_peg_assignment_operator)
  (railroad_peg_choice_operator)
  (railroad_peg_prefix_operator)
  (railroad_peg_suffix_operator)
] @operator

(railroad_peg_string) @string
(railroad_peg_any) @constant
(railroad_peg_comment) @comment
