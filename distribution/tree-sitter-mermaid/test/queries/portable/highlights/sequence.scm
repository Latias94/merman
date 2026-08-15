; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(sequence_statement_keyword) @keyword
(sequence_block_keyword) @keyword
(sequence_block_end) @keyword

(sequence_participant_name) @type
(sequence_actor_reference) @variable
(sequence_participant_config) @attribute
(sequence_number) @number

[
  (sequence_message_operator)
  (sequence_central_connection)
  (sequence_inline_activation)
] @operator

(sequence_note_placement) @keyword.operator

[
  (sequence_line_text)
  (sequence_message_text)
  (sequence_note_text)
  (sequence_block_label)
] @string
