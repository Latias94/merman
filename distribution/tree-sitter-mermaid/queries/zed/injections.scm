; Zed injection profile. Host Markdown grammars own Mermaid fence injection;
; this query only exposes embedded payloads inside a Mermaid source file.

((frontmatter) @injection.content
  (#set! injection.language "yaml"))

([
  (xy_chart_markdown_content)
  (xy_chart_markdown_backtick_content)
] @injection.content
  (#set! injection.language "markdown-inline"))
