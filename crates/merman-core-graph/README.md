# merman-core-graph

Syntax parsing for Mermaid graph diagrams. `diagram-class` produces source-ordered grammar actions.
`diagram-er` parses entity–relationship diagrams into canonical `merman-model` values. `diagram-state`
produces a typed nested document. `editor-facts` exposes lexical events for editor consumers.

Use `merman-core` to parse a complete Mermaid source and map diagnostics to the original input.
