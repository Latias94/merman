# merman-render-structured

Layout and SVG emission for structured Mermaid diagrams. The `diagram-info`, `diagram-packet`, and
`diagram-railroad` features provide typed layout, presentation binding, and SVG contents for their
families. `merman-render` selects the family and applies the document viewport and output policies.

The `diagram-ishikawa` feature provides the typed layout and configuration projection.
`merman-render` supplies text measurement, binds the theme, and writes the SVG.

Use `merman` for source-to-output rendering.
