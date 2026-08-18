; Aerial-compatible @symbol/@name captures. The diagram itself is always a
; navigable module; nested declarations enrich structured families.

(source_file
  (_
    header: (_
      keyword: (diagram_keyword) @name)) @symbol
  (#set! "kind" "Module"))

(architecture_group_statement
  id: (architecture_identifier) @name
  (#set! "kind" "Namespace")) @symbol

[
  (architecture_service_statement
    id: (architecture_identifier) @name
    (#set! "kind" "Object"))
  (architecture_junction_statement
    id: (architecture_identifier) @name
    (#set! "kind" "Object"))
] @symbol

(class_namespace_declaration
  name: (class_namespace_name) @name
  (#set! "kind" "Namespace")) @symbol

(class_declaration
  name: (class_name) @name
  (#set! "kind" "Class")) @symbol

(er_entity_declaration
  name: (er_entity_name) @name
  (#set! "kind" "Class")) @symbol

(event_entity_statement
  name: (event_qualified_name) @name
  (#set! "kind" "Class")) @symbol

(event_data_statement
  name: (event_data_name) @name
  (#set! "kind" "Variable")) @symbol

(gantt_section_statement
  name: (gantt_line_text) @name
  (#set! "kind" "Namespace")) @symbol

(gantt_task_statement
  name: (gantt_task_name) @name
  (#set! "kind" "Variable")) @symbol

(journey_section_statement
  section: (journey_section_name) @name
  (#set! "kind" "Namespace")) @symbol

(railroad_rule
  name: (railroad_identifier) @name
  (#set! "kind" "Function")) @symbol

(railroad_abnf_rule
  name: (railroad_abnf_rule_name) @name
  (#set! "kind" "Function")) @symbol

(railroad_ebnf_rule
  name: (railroad_ebnf_identifier) @name
  (#set! "kind" "Function")) @symbol

(railroad_peg_rule
  name: (railroad_peg_identifier) @name
  (#set! "kind" "Function")) @symbol

[
  (requirement_declaration
    name: (requirement_name) @name
    (#set! "kind" "Object"))
  (requirement_element_declaration
    name: (requirement_name) @name
    (#set! "kind" "Object"))
] @symbol

(sequence_participant_declaration
  name: (sequence_participant_name) @name
  (#set! "kind" "Object")) @symbol

[
  (state_named_declaration
    name: (state_name) @name
    (#set! "kind" "Object"))
  (state_pseudostate_declaration
    name: (state_name) @name
    (#set! "kind" "Object"))
] @symbol

(state_composite_declaration
  name: (state_name) @name
  (#set! "kind" "Namespace")) @symbol

(timeline_section_statement
  name: (timeline_section_name) @name
  (#set! "kind" "Namespace")) @symbol

[
  (wardley_component_statement
    name: (wardley_name) @name
    (#set! "kind" "Object"))
  (wardley_anchor_statement
    name: (wardley_name) @name
    (#set! "kind" "Object"))
] @symbol

[
  (zenuml_starter_declaration
    participant: (zenuml_name) @name
    (#set! "kind" "Object"))
  (zenuml_participant_declaration
    name: (zenuml_name) @name
    (#set! "kind" "Object"))
] @symbol
