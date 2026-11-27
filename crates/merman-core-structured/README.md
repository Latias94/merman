# merman-core-structured

Source parsing and semantic construction for structured Mermaid diagrams. `diagram-cynefin`
parses domain items and transitions. `diagram-gantt` scans Gantt statements, resolves dates
with the supplied operation context, and builds typed task models. `diagram-info` scans Info
headers and statements and builds the typed model.
`diagram-ishikawa` scans effects and causes and builds the flat cause tree. `diagram-journey`
parses Journey statements and builds the typed task and section model. `diagram-packet` scans
Packet headers and fields and expands them into bounded rows. `diagram-pie` parses chart
statements and builds the ordered slice model. `diagram-quadrant-chart` builds axes, points,
and their styles. `diagram-radar` resolves axes, curves, and options. `diagram-railroad`
parses Railroad IR, EBNF, ABNF, and PEG into `merman-model` values. `diagram-sankey` parses
CSV and builds the canonical graph. `diagram-tree-view` parses indented and box-drawing trees.
`diagram-treemap` parses rows and builds the Treemap hierarchy. `diagram-venn` parses Venn
statements and builds the typed set model. `diagram-xychart` parses chart statements and builds
typed axes and series.
`editor-facts` enables syntax spans for editor consumers.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` to parse a complete Mermaid source with frontmatter, configuration, diagnostic
mapping, and editor facts.
