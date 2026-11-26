# merman-render-structured

Layout and SVG emission for structured Mermaid diagrams. The `diagram-info` feature provides typed
layout, presentation binding, and complete Info SVG emission. `merman-render` supplies the root
context and applies canvas and terminal output policies.

The `diagram-packet` feature provides typed layout, presentation binding, and complete Packet SVG
emission. `merman-render` supplies the root context and applies canvas and terminal output policies.

The `diagram-railroad` feature provides typed layout, presentation binding, and SVG contents.
`merman-render` selects the family and applies the document viewport and output policies.

The `diagram-ishikawa` feature provides typed layout, presentation binding, and complete classic
and hand-drawn SVG emission. `merman-render` supplies the operation's text measurer, theme
selection, and root context, then applies canvas and terminal output policies.

Use `merman` for source-to-output rendering.
