# merman-render-structured

Layout and SVG emission for structured Mermaid diagrams. The `diagram-info` feature provides typed
layout, presentation binding, and complete Info SVG emission. `merman-render` supplies the root
context and applies canvas and terminal output policies.

The `diagram-packet` and `diagram-railroad` features provide typed layout, presentation binding,
and SVG contents for their families. `merman-render` selects the family and applies the document
viewport and output policies.

The `diagram-ishikawa` feature provides typed layout, configuration projection, and SVG contents.
`merman-render` supplies text measurement, resolves the theme, and assembles the SVG document.

Use `merman` for source-to-output rendering.
