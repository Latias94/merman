; Helix 25.07.1 consumes this file through SyntaxConfig::new.
; Frontmatter content excludes Mermaid's --- delimiters, so the combined ranges
; form a valid YAML document for every Mermaid family.

((frontmatter_content) @injection.content
 (#set! injection.language "yaml")
 (#set! injection.combined))

((comment) @injection.content
 (#set! injection.language "comment"))

; Markdown-looking Mermaid labels are intentionally not injected yet. Most
; family CST nodes still own their quote/backtick delimiters, and Helix does
; not implement Neovim's query offset directive. Injecting those nodes would
; hand invalid Markdown ranges to the child parser.
