(diagram_keyword) @keyword
(railroad_statement_keyword) @keyword
(railroad_line_text) @string

(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @function)

(railroad_ebnf_reference
  name: (railroad_ebnf_identifier) @variable)

[
  (railroad_ebnf_assignment_operator)
  (railroad_ebnf_choice_operator)
  (railroad_ebnf_quantifier)
  (railroad_ebnf_exception_operator)
] @operator

(railroad_ebnf_string) @string
(railroad_ebnf_special_text) @string.special
(railroad_ebnf_iso_comment) @comment
(railroad_ebnf_block_comment) @comment
