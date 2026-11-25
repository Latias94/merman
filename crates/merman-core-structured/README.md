# merman-core-structured

Syntax parsing for structured Mermaid diagrams. The `diagram-railroad` feature parses Railroad
IR, EBNF, ABNF, and PEG into `merman-model` values and locates syntax errors in the input. The
`editor-facts` feature allows callers to request syntax spans for recovered rules and predicates.

Use `merman-core` to parse a complete Mermaid source with frontmatter, configuration, diagnostic
mapping, and editor facts.
