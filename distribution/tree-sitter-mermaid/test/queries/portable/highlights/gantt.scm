; Standalone family fragment. Keep the central diagram_keyword capture once.
(diagram_keyword) @keyword
(statement_keyword) @keyword

(gantt_task_status) @attribute
(gantt_constraint_keyword) @keyword.operator
(gantt_action_keyword) @keyword
(gantt_weekday) @constant
(gantt_weekend_day) @constant

(gantt_title_statement text: (gantt_line_text) @string)
(gantt_section_statement name: (gantt_line_text) @string)
(gantt_task_name) @string
(gantt_setting_value) @string
(gantt_today_marker_value) @string
(gantt_accessibility_block_text) @string
(gantt_unclosed_accessibility_block_text) @string

(gantt_date) @number
(gantt_duration) @number
(gantt_reference) @variable
(gantt_callback_name) @function
(gantt_callback_arguments) @string
(gantt_url) @string.special
(gantt_unclosed_url) @string.special

(gantt_task_statement delimiter: ":" @punctuation.delimiter)
(gantt_task_metadata "," @punctuation.delimiter)
(gantt_call_action ["(" ")"] @punctuation.bracket)
