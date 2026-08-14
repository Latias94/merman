; Portable mechanics profile. Family-complete highlight coverage is admitted in U8.

(diagram_keyword) @keyword
(comment) @comment
(directive) @attribute
(frontmatter_delimiter) @punctuation.special
(quoted_string) @string
(number) @number

; Shared structured-family vocabulary.
(statement_keyword) @keyword

[
  (langium_string)
  (langium_line_text)
  (langium_acc_descr_block_text)
] @string

; Architecture.
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

; Cynefin.
(cynefin_domain_name) @keyword
(cynefin_transition_operator) @operator

; GitGraph.
(git_graph_statement_keyword) @keyword
(git_graph_clause_keyword) @property

[
  (git_graph_header_separator)
  (git_graph_clause_separator)
] @punctuation.delimiter

[
  (git_graph_direction)
  (git_graph_commit_type)
] @constant

(git_graph_reference) @variable
(git_graph_integer) @number

; Packet.
[
  (packet_range_operator)
  (packet_width_operator)
] @operator

(packet_label_delimiter) @punctuation.delimiter
(packet_integer) @number

; Pie.
(pie_show_data_option) @keyword
(pie_section_delimiter) @punctuation.delimiter
(pie_number) @number

; Radar.
(radar_axis
  name: (radar_identifier) @variable)

(radar_curve
  name: (radar_identifier) @function)

(radar_detailed_entry
  axis: (radar_identifier) @variable)

(radar_option
  name: (radar_option_name) @property)

[
  (radar_title_text)
  (radar_accessibility_text)
  (radar_accessibility_block)
] @string

(radar_number) @number
(radar_boolean) @boolean
(radar_graticule) @constant

; Wardley.
(wardley_component_statement
  name: (wardley_name) @variable)

(wardley_anchor_statement
  name: (wardley_name) @variable)

(wardley_link_statement
  source: (wardley_name) @variable
  target: (wardley_name) @variable)

(wardley_evolve_statement
  component: (wardley_name) @variable)

(wardley_pipeline_statement
  parent: (wardley_name) @variable)

(wardley_pipeline_component_statement
  name: (wardley_name) @variable)

[
  (wardley_arrow)
  (wardley_link_operator)
  (wardley_link_port)
] @operator

(wardley_strategy) @constant

[
  (wardley_title_text)
  (wardley_accessibility_text)
  (wardley_accessibility_block)
  (wardley_link_label_value)
] @string

[
  (wardley_decimal)
  (wardley_integer)
  (wardley_signed_integer)
] @number
