; Markdown payloads use the Neovim runtime's markdown_inline parser. Event
; Modeling payloads are mapped from Mermaid's external-language tag to a
; concrete installed Tree-sitter language instead of treating arbitrary text
; as an executable injection.

([
  (flow_markdown_label)
  (swimlane_markdown_label)
] @injection.content
  (#offset! @injection.content 0 2 0 -2)
  (#set! injection.language "markdown_inline"))

([
  (mindmap_markdown_string)
  (kanban_markdown_string)
] @injection.content
  (#match? @injection.content "^\"`")
  (#offset! @injection.content 0 2 0 -2)
  (#set! injection.language "markdown_inline"))

([
  (mindmap_markdown_string)
  (kanban_markdown_string)
] @injection.content
  (#match? @injection.content "^`")
  (#offset! @injection.content 0 1 0 -1)
  (#set! injection.language "markdown_inline"))

((xy_chart_markdown_text
   (xy_chart_markdown_content) @injection.content)
  (#set! injection.language "markdown_inline")
  (#set! injection.combined))

((event_inline_data
   type: (event_data_type
     kind: (event_data_type_name) @_event_language)
   value: (event_inline_object) @injection.content)
  (#any-of? @_event_language "json" "jsobj" "figma")
  (#set! injection.language "json"))

((event_data_block
   type: (event_data_type
     kind: (event_data_type_name) @_event_language)
   content: [
     (event_data_fragment)
     (event_nested_data_block)
   ] @injection.content)
  (#any-of? @_event_language "json" "jsobj" "figma")
  (#set! injection.language "json")
  (#set! injection.combined))

((event_inline_data
   type: (event_data_type
     kind: (event_data_type_name) @_event_language)
   value: (_) @injection.content)
  (#eq? @_event_language "md")
  (#set! injection.language "markdown_inline"))

((event_data_block
   type: (event_data_type
     kind: (event_data_type_name) @_event_language)
   content: [
     (event_data_fragment)
     (event_nested_data_block)
   ] @injection.content)
  (#eq? @_event_language "md")
  (#set! injection.language "markdown_inline")
  (#set! injection.combined))

((event_inline_data
   type: (event_data_type
     kind: (event_data_type_name) @_event_language)
   value: (_) @injection.content)
  (#eq? @_event_language "html")
  (#set! injection.language "html"))

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
