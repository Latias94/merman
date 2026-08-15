(diagram_keyword) @keyword
(railroad_statement_keyword) @keyword
(railroad_line_text) @string

(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @function)

(railroad_abnf_reference
  name: (railroad_abnf_rule_name) @variable)

[
  (railroad_abnf_assignment_operator)
  (railroad_abnf_alternation_operator)
] @operator

(railroad_abnf_repeat) @number
(railroad_abnf_string) @string
(railroad_abnf_numeric_value) @number
(railroad_abnf_comment) @comment
