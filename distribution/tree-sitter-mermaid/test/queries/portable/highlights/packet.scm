; This fragment is standalone for its family golden. When merging it into
; queries/portable/highlights.scm, retain the central diagram_keyword capture
; only once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

[
  (packet_range_operator)
  (packet_width_operator)
] @operator

(packet_label_delimiter) @punctuation.delimiter
(packet_integer) @number

[
  (langium_string)
  (langium_line_text)
  (langium_acc_descr_block_text)
] @string
