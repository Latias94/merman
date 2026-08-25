; Portable injections require delimiter-free payload nodes and standard query
; predicates. Families that require editor-specific offsets remain N/A.

; Event Modeling typed data blocks.
((event_data_block
  type: (event_data_type
    kind: (event_data_type_name) @_event_language)
  content: [
    (event_data_fragment)
    (event_nested_data_block)
  ] @injection.content)
  (#eq? @_event_language "json")
  (#set! injection.language "json")
  (#set! injection.combined))

((event_data_block
  type: (event_data_type
    kind: (event_data_type_name) @_event_language)
  content: [
    (event_data_fragment)
    (event_nested_data_block)
  ] @injection.content)
  (#eq? @_event_language "md")
  (#set! injection.language "markdown")
  (#set! injection.combined))

((event_data_block
  type: (event_data_type
    kind: (event_data_type_name) @_event_language)
  content: [
    (event_data_fragment)
    (event_nested_data_block)
  ] @injection.content)
  (#eq? @_event_language "html")
  (#set! injection.language "html")
  (#set! injection.combined))

; XY Chart Markdown text already exposes its delimiters separately.
((xy_chart_markdown_text
  [
    (xy_chart_markdown_content)
    (xy_chart_markdown_backtick_content)
  ] @injection.content)
  (#set! injection.language "markdown")
  (#set! injection.combined))
