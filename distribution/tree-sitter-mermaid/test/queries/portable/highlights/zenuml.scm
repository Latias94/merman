; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(zenuml_statement_keyword) @keyword
(zenuml_control_keyword) @keyword
(zenuml_modifier) @keyword

[
  (zenuml_starter_annotation)
  (zenuml_reply_annotation)
  (zenuml_participant_annotation)
  (zenuml_stereotype)
  (zenuml_color)
] @attribute

(zenuml_participant_declaration
  name: (zenuml_name) @type)
(zenuml_starter_declaration
  participant: (zenuml_name) @type)
(zenuml_construct
  name: (zenuml_name) @type)

(zenuml_endpoint
  name: (zenuml_name) @variable)
(zenuml_reference_list
  participant: (zenuml_name) @variable)
(zenuml_assignee
  item: (_) @variable)
(zenuml_expression
  (zenuml_identifier) @variable)

(zenuml_signature
  name: (zenuml_name) @function)
(zenuml_named_argument
  name: (zenuml_identifier) @property)

[
  (zenuml_arrow)
  (zenuml_return_arrow)
  (zenuml_operator)
  (zenuml_assignment_operator)
] @operator

[
  (zenuml_title_text)
  (zenuml_event_payload)
  (zenuml_divider_text)
  (zenuml_string)
  (zenuml_unclosed_string)
] @string

[
  (zenuml_number)
  (zenuml_number_unit)
  (zenuml_money)
] @number

(zenuml_boolean) @boolean
(zenuml_nil) @constant
(zenuml_emoji) @string.special
(zenuml_comment) @comment
