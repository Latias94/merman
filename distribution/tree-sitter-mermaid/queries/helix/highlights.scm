; Helix 25.07.1 highlight profile. Keep capture names in the Helix theme scope vocabulary.

(diagram_keyword) @keyword
(comment) @comment
(directive) @attribute
(frontmatter_delimiter) @punctuation.special
(quoted_string) @string

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
(git_graph_clause_keyword) @variable.other.member

[
  (git_graph_header_separator)
  (git_graph_clause_separator)
] @punctuation.delimiter

[
  (git_graph_direction)
  (git_graph_commit_type)
] @constant

(git_graph_reference) @variable
(git_graph_integer) @constant.numeric

; Packet.
[
  (packet_range_operator)
  (packet_width_operator)
] @operator

(packet_label_delimiter) @punctuation.delimiter
(packet_integer) @constant.numeric

; Pie.
(pie_show_data_option) @keyword
(pie_section_delimiter) @punctuation.delimiter
(pie_number) @constant.numeric

; Radar.
(radar_axis
  name: (radar_identifier) @variable)

(radar_curve
  name: (radar_identifier) @function)

(radar_detailed_entry
  axis: (radar_identifier) @variable)

(radar_option
  name: (radar_option_name) @variable.other.member)

[
  (radar_title_text)
  (radar_accessibility_text)
  (radar_accessibility_block)
] @string

(radar_number) @constant.numeric
(radar_boolean) @constant.builtin.boolean
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
] @constant.numeric

; Gantt.
(gantt_task_status) @attribute
(gantt_constraint_keyword) @keyword.operator
(gantt_action_keyword) @keyword

[
  (gantt_weekday)
  (gantt_weekend_day)
] @constant

(gantt_title_statement text: (gantt_line_text) @string)
(gantt_section_statement name: (gantt_line_text) @string)

[
  (gantt_task_name)
  (gantt_setting_value)
  (gantt_today_marker_value)
  (gantt_accessibility_block_text)
  (gantt_unclosed_accessibility_block_text)
] @string

[
  (gantt_date)
  (gantt_duration)
] @constant.numeric

(gantt_reference) @variable
(gantt_callback_name) @function
(gantt_callback_arguments) @string

[
  (gantt_url)
  (gantt_unclosed_url)
] @string.special

(gantt_task_statement delimiter: ":" @punctuation.delimiter)
(gantt_task_metadata "," @punctuation.delimiter)
(gantt_call_action ["(" ")"] @punctuation.bracket)

; Ishikawa.
(ishikawa_label) @string

; Journey.
(journey_section_name) @namespace
(journey_task_name) @string
(journey_score) @constant.numeric
(journey_actor) @variable

[
  (journey_title_text)
  (journey_accessibility_line_text)
  (journey_accessibility_description_block)
  (journey_unclosed_accessibility_description_block)
] @string

[
  (journey_task_delimiter)
  (journey_actor_delimiter)
] @punctuation.delimiter

(journey_hash_comment) @comment

; Quadrant Chart.
[
  (quadrant_chart_axis)
  (quadrant_chart_quadrant)
] @keyword

(quadrant_chart_axis_delimiter) @operator

[
  (quadrant_chart_line_text)
  (quadrant_chart_accessibility_line_text)
  (quadrant_chart_accessibility_description_block)
  (quadrant_chart_unclosed_accessibility_description_block)
  (quadrant_chart_axis_text)
  (quadrant_chart_label)
  (quadrant_chart_point_label)
  (quadrant_chart_style_value)
] @string

(quadrant_chart_class_name) @type
(quadrant_chart_style_name) @variable.other.member

[
  (quadrant_chart_coordinate)
  (quadrant_chart_invalid_coordinate)
] @constant.numeric

[
  (quadrant_chart_point_delimiter)
  (quadrant_chart_class_delimiter)
] @punctuation.delimiter

(quadrant_chart_coordinates
  ["[" "]"] @punctuation.bracket
  "," @punctuation.delimiter)

(quadrant_chart_style
  ":" @punctuation.delimiter)

(quadrant_chart_style_list
  "," @punctuation.delimiter)

; Requirement.
(requirement_statement_keyword) @keyword
(requirement_attribute_keyword) @variable.other.member
(requirement_kind) @type
(requirement_relationship_kind) @keyword.operator

[
  (requirement_direction)
  (requirement_risk)
  (requirement_verify_method)
] @constant

[
  (requirement_unquoted_name)
  (requirement_unquoted_reference)
  (requirement_style_identifier)
] @variable

[
  (requirement_string)
  (requirement_unclosed_string)
  (requirement_attribute_text)
  (requirement_line_text)
  (requirement_accessibility_block_text)
  (requirement_style_value)
] @string

(requirement_style_property) @variable.other.member
(requirement_relationship_operator) @operator
(requirement_hash_comment) @comment

(requirement_attribute
  delimiter: ":" @punctuation.delimiter)

(requirement_element_attribute
  delimiter: ":" @punctuation.delimiter)

(requirement_style_declaration
  delimiter: ":" @punctuation.delimiter)

(requirement_class_annotation
  delimiter: [":::" ","] @punctuation.delimiter)

(requirement_identifier_list
  delimiter: "," @punctuation.delimiter)

(requirement_declaration
  open: "{" @punctuation.bracket
  close: "}" @punctuation.bracket)

(requirement_element_declaration
  open: "{" @punctuation.bracket
  close: "}" @punctuation.bracket)

; Timeline.
(timeline_statement_keyword) @keyword
(timeline_direction) @constant

[
  (timeline_line_text)
  (timeline_section_name)
  (timeline_period)
  (timeline_event_text)
  (timeline_accessibility_block_text)
] @string

(timeline_event_delimiter) @punctuation.delimiter
(timeline_hash_comment) @comment

; XY Chart.
(xy_chart_beta_marker) @attribute
(xy_chart_orientation) @constant

[
  (xy_chart_quoted_text)
  (xy_chart_markdown_text)
  (xy_chart_bare_text)
  (xy_chart_accessibility_text)
  (xy_chart_accessibility_block_text)
] @string

(xy_chart_axis_range
  (xy_chart_number) @constant.numeric)

(xy_chart_incomplete_axis_range
  (xy_chart_number) @constant.numeric)

(xy_chart_data_point
  value: (xy_chart_number) @constant.numeric)

(xy_chart_range_delimiter) @operator

[
  (xy_chart_array_open)
  (xy_chart_array_close)
] @punctuation.bracket

[
  (xy_chart_array_delimiter)
  (xy_chart_accessibility_delimiter)
(xy_chart_statement_delimiter)
] @punctuation.delimiter

; Block.
(block_statement_keyword) @keyword
(block_end) @keyword

(block_identifier) @variable
(block_arrow_direction) @constant

[
  (block_quoted_label)
  (block_bare_label)
  (block_line_text)
  (block_accessibility_description_block)
  (block_unclosed_accessibility_description_block)
  (block_style_value)
] @string

[
  (block_column_count)
  (block_width)
] @constant.numeric

(block_style_property) @variable.other.member

[
  (block_edge_label_start)
  (block_edge_operator)
] @operator

(block_shape_delimiter) @punctuation.bracket

(block_space_statement delimiter: ":" @punctuation.delimiter)
(block_width_clause delimiter: ":" @punctuation.delimiter)
(block_identifier_list delimiter: "," @punctuation.delimiter)
(block_style_list delimiter: "," @punctuation.delimiter)
(block_style_declaration delimiter: ":" @punctuation.delimiter)

; C4.
(c4_statement_keyword) @keyword
(c4_entity_kind) @type
(c4_boundary_kind) @type
(c4_relationship_kind) @keyword.operator
(c4_update_kind) @function.macro
(c4_direction) @constant

(c4_identifier) @variable
(c4_property_name) @variable.other.member

[
  (c4_string)
  (c4_unclosed_string)
  (c4_unquoted_argument)
  (c4_line_text)
  (c4_accessibility_description_block)
  (c4_unclosed_accessibility_description_block)
] @string

(c4_named_argument sigil: "$" @punctuation.special)
(c4_named_argument operator: "=" @operator)

[
  (c4_entity_declaration open: "(")
  (c4_entity_declaration close: ")")
  (c4_boundary_statement open: "{")
  (c4_boundary_statement close: "}")
  (c4_relationship_statement open: "(")
  (c4_relationship_statement close: ")")
  (c4_style_update_statement open: "(")
  (c4_style_update_statement close: ")")
] @punctuation.bracket

[
  (c4_entity_declaration delimiter: ",")
  (c4_boundary_statement delimiter: ",")
  (c4_relationship_statement delimiter: ",")
  (c4_style_update_statement delimiter: ",")
] @punctuation.delimiter

; Class.
(class_statement_keyword) @keyword
(class_callback_keyword) @keyword

(class_namespace_name) @namespace
(class_direction) @constant

[
  (class_name)
  (class_reference)
] @type

[
  (class_style_name)
  (class_annotation_name)
] @attribute

(class_member) @variable.other.member
(class_style_item) @variable.other.member
(class_relationship_operator) @operator
(class_relationship_label) @string
(class_note_relation) @keyword.operator

[
  (class_string)
  (class_unclosed_string)
  (class_note_text)
  (class_line_text)
  (class_accessibility_description_block)
  (class_unclosed_accessibility_description_block)
] @string

(class_callback_name) @function
(class_callback_arguments) @string
(class_link_target) @constant

; Entity Relationship.
(er_statement_keyword) @keyword

[
  (er_entity_name)
  (er_entity_reference)
] @type

(er_attribute_type) @type.builtin
(er_attribute_name) @variable.other.member
(er_attribute_key) @attribute
(er_direction) @constant

[
  (er_cardinality)
  (er_relationship_operator)
] @operator

[
  (er_quoted_text)
  (er_unclosed_quoted_text)
  (er_role_text)
  (er_line_text)
  (er_accessibility_description_block)
  (er_unclosed_accessibility_description_block)
] @string

(er_style_name) @attribute
(er_style_item) @variable.other.member

; Flowchart.
(flow_statement_keyword) @keyword
(flow_subgraph_end) @keyword

[
  (flow_node_id)
  (flow_reference)
] @variable

(flow_edge_name) @variable.other.member
(flow_class_name) @type
(flow_callback_name) @function

[
  (flow_quoted_label)
  (flow_markdown_label)
  (flow_label_text)
  (flow_square_label_text)
  (flow_round_label_text)
  (flow_curly_label_text)
  (flow_edge_label_text)
  (flow_middle_edge_label_text)
  (flow_shape_data_string)
  (flow_style_value)
  (flow_accessibility_text)
  (flow_accessibility_block_text)
] @string

[
  (direction)
  (flow_direction)
  (flow_link_target)
] @constant

(flow_style_property) @variable.other.member
(flow_edge_index) @constant.numeric

[
  (flow_arrow)
  (flow_arrow_start)
  (flow_continued_arrow)
  (flow_continued_arrow_start)
] @operator

(flow_shape_delimiter) @punctuation.bracket

(flow_edge_id delimiter: "@" @punctuation.delimiter)
(flow_edge_label open: "|" @punctuation.delimiter)
(flow_edge_label close: "|" @punctuation.delimiter)
(flow_identifier_list delimiter: "," @punctuation.delimiter)
(flow_number_list delimiter: "," @punctuation.delimiter)
(flow_style_list delimiter: "," @punctuation.delimiter)
(flow_style_declaration delimiter: ":" @punctuation.delimiter)

; State.
(state_statement_keyword) @keyword
(state_note_end) @keyword

[
  (state_name)
  (state_reference)
] @variable

(state_class_name) @type

[
  (state_quoted_text)
  (state_description_text)
  (state_note_text)
  (state_note_line)
  (state_style_value)
  (state_accessibility_text)
  (state_accessibility_block_text)
] @string

[
  (state_pseudostate_kind)
  (state_marker)
  (state_direction)
  (state_note_position)
] @constant

(state_style_property) @variable.other.member
(state_scale_width) @constant.numeric

[
  (state_transition_operator)
  (state_concurrent_divider)
] @operator

(state_class_annotation operator: ":::" @operator)

(state_transition_statement delimiter: ":" @punctuation.delimiter)
(state_description_statement delimiter: ":" @punctuation.delimiter)
(state_inline_note delimiter: ":" @punctuation.delimiter)
(state_alias_declaration delimiter: ":" @punctuation.delimiter)
(state_identifier_list delimiter: "," @punctuation.delimiter)
(state_style_list delimiter: "," @punctuation.delimiter)
(state_style_declaration delimiter: ":" @punctuation.delimiter)

; Swimlane. The shared direction node is captured once in the Flowchart section.
(swimlane_statement_keyword) @keyword
(swimlane_subgraph_end) @keyword

[
  (swimlane_node_id)
  (swimlane_reference)
] @variable

(swimlane_edge_name) @variable.other.member
(swimlane_class_name) @type
(swimlane_callback_name) @function

[
  (swimlane_quoted_label)
  (swimlane_markdown_label)
  (swimlane_label_text)
  (swimlane_square_label_text)
  (swimlane_round_label_text)
  (swimlane_curly_label_text)
  (swimlane_edge_label_text)
  (swimlane_middle_edge_label_text)
  (swimlane_shape_data_string)
  (swimlane_style_value)
  (swimlane_accessibility_text)
  (swimlane_accessibility_block_text)
] @string

[
  (swimlane_direction)
  (swimlane_link_target)
] @constant

(swimlane_style_property) @variable.other.member
(swimlane_edge_index) @constant.numeric

[
  (swimlane_arrow)
  (swimlane_arrow_start)
  (swimlane_continued_arrow)
  (swimlane_continued_arrow_start)
] @operator

(swimlane_shape_delimiter) @punctuation.bracket

(swimlane_edge_id delimiter: "@" @punctuation.delimiter)
(swimlane_edge_label open: "|" @punctuation.delimiter)
(swimlane_edge_label close: "|" @punctuation.delimiter)
(swimlane_identifier_list delimiter: "," @punctuation.delimiter)
(swimlane_number_list delimiter: "," @punctuation.delimiter)
(swimlane_style_list delimiter: "," @punctuation.delimiter)
(swimlane_style_declaration delimiter: ":" @punctuation.delimiter)

; Event Modeling.
(event_statement_keyword) @keyword

(event_frame_id) @constant.numeric
(event_entity_kind) @type.builtin
(event_name_part) @type
(event_data_name) @variable
(event_relation_operator) @operator
(event_data_type_name) @type.builtin

[
  (event_inline_object)
  (event_inline_string)
  (event_line_text)
] @string

; Sequence.
(sequence_statement_keyword) @keyword
(sequence_block_keyword) @keyword
(sequence_block_end) @keyword

(sequence_participant_name) @type
(sequence_actor_reference) @variable
(sequence_participant_config) @attribute
(sequence_number) @constant.numeric

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

; Kanban.
(kanban_item
  id: (kanban_item_id) @variable)

[
  (kanban_plain_label)
  (kanban_label_text)
  (kanban_quoted_string)
  (kanban_markdown_string)
  (kanban_multiline_label_text)
] @string

(kanban_metadata_pair
  key: (kanban_metadata_key) @variable.other.member)

(kanban_metadata_bare_value) @string

(kanban_icon_marker) @function.macro
(kanban_icon_name) @string.special
(kanban_class_marker) @punctuation.special
(kanban_class_list) @type

(kanban_shape_delimiter) @punctuation.bracket
(kanban_metadata_delimiter) @punctuation.bracket
(kanban_metadata_separator) @punctuation.delimiter

; Mindmap.
(mindmap_node
  id: (mindmap_node_id) @variable)

[
  (mindmap_plain_label)
  (mindmap_label_text)
  (mindmap_quoted_string)
  (mindmap_markdown_string)
  (mindmap_multiline_label_text)
] @string

(mindmap_icon_marker) @function.macro
(mindmap_icon_name) @string.special
(mindmap_class_marker) @punctuation.special
(mindmap_class_list) @type
(mindmap_shape_delimiter) @punctuation.bracket

; Sankey.
(sankey_record source: (sankey_field) @string)
(sankey_record target: (sankey_field) @string)
(sankey_record value: (sankey_field) @constant.numeric)

(sankey_escaped_quote) @constant.character.escape
(sankey_quote) @punctuation.bracket
(sankey_record_delimiter) @punctuation.delimiter

; Venn.
(venn_set_expression set: (venn_identifier) @variable)
(venn_intersection_expression set: (venn_identifier) @variable)
(venn_expression (venn_identifier) @variable)
(venn_text_value (venn_identifier) @string)

(venn_title_text) @string
(venn_label
  text: [
    (venn_quoted_label)
    (venn_unquoted_label)
  ] @string)

(venn_number) @constant.numeric
(venn_color) @string.special
(venn_style_property) @variable.other.member
(venn_style_atom) @constant

(venn_quote) @punctuation.bracket
(venn_label_delimiter) @punctuation.bracket
(venn_set_delimiter) @punctuation.delimiter
(venn_value_delimiter) @punctuation.delimiter
(venn_style_delimiter) @punctuation.delimiter

; Tree View.
[
  (tree_view_bare_name)
  (tree_view_quoted_name)
  (tree_view_unclosed_name)
] @string

(tree_view_class_marker) @punctuation.special
(tree_view_class_name) @type
(tree_view_icon_open) @function.macro
(tree_view_icon_name) @string.special
(tree_view_description_marker) @punctuation.special
(tree_view_description_text) @comment.block.documentation

[
  (tree_view_box_prefix)
  (tree_view_box_decoration)
] @punctuation.special

; Treemap.
[
  (treemap_quoted_name)
  (treemap_unclosed_name)
] @string

(treemap_value_separator) @punctuation.delimiter
(treemap_number) @constant.numeric
(treemap_class_marker) @punctuation.special
(treemap_class_name) @type

; Railroad shared.
(railroad_statement_keyword) @keyword
(railroad_line_text) @string

; Railroad IR.
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

; Railroad ABNF.
(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @function)

(railroad_abnf_reference
  name: (railroad_abnf_rule_name) @variable)

[
  (railroad_abnf_assignment_operator)
  (railroad_abnf_alternation_operator)
] @operator

(railroad_abnf_repeat) @constant.numeric
(railroad_abnf_string) @string
(railroad_abnf_numeric_value) @constant.numeric
(railroad_abnf_comment) @comment

; Railroad EBNF.
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

; Railroad PEG.
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

; ZenUML.
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
  name: (zenuml_identifier) @variable.other.member)

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
] @constant.numeric

(zenuml_boolean) @constant.builtin.boolean
(zenuml_nil) @constant
(zenuml_emoji) @string.special
(zenuml_comment) @comment
