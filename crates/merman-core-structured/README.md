# merman-core-structured

Syntax parsing for structured Mermaid diagrams. The `diagram-info` feature scans Info headers
and statements. The `diagram-packet` feature scans Packet headers and fields and expands them
into bounded rows. The `diagram-railroad` feature parses Railroad IR, EBNF, ABNF, and PEG into
`merman-model` values. The `editor-facts` feature enables syntax spans for editor consumers.

Use `merman-core` to parse a complete Mermaid source with frontmatter, configuration, diagnostic
mapping, and editor facts.
