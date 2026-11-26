# merman-render-structured

Layout and SVG emission for structured Mermaid diagrams. The `diagram-info` feature provides typed
layout, presentation binding, and complete Info SVG emission. `merman-render` supplies the root
context and applies canvas and terminal output policies.

The `diagram-packet` feature provides typed layout, presentation binding, and complete Packet SVG
emission. `merman-render` supplies the root context and applies canvas and terminal output policies.

The `diagram-railroad` feature provides typed layout, presentation binding, and SVG contents.
`merman-render` selects the family and applies the document viewport and output policies.

The `diagram-ishikawa` feature provides typed layout, configuration projection, and SVG contents.
`merman-render` supplies text measurement, resolves the theme, and assembles the SVG document.

Use `merman` for source-to-output rendering.
