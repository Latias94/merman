# merman-core-graph

Parsing for Mermaid graph diagrams. `diagram-class` produces source-ordered grammar actions.
`diagram-er` parses entity–relationship diagrams into canonical `merman-model` values.
`diagram-git-graph` owns GitGraph syntax and semantic model construction. `diagram-state` owns
State syntax and semantic model construction, including a typed render model with `presentation`.
`editor-facts` exposes lexical events for editor consumers.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` to parse a complete Mermaid source and map diagnostics to the original input.
