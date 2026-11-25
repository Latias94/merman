# merman-render-charts

The `diagram-pie` feature provides typed layout, theme binding, and SVG content for Pie diagrams.
`merman-render` supplies the operation's text measurement, root viewport, and SVG output policy.

The `diagram-quadrant-chart` feature provides typed geometry and paint binding for Quadrant
Chart. `merman-render` resolves native theme precedence and emits the SVG.

The `diagram-radar` feature provides typed layout and the point and radius codecs for Radar.
`merman-render` resolves the theme and writes the SVG.

The `diagram-sankey` feature provides typed layout and configuration lookup. `merman-render`
binds the theme, measures labels, and writes the SVG.

The `diagram-treemap` feature provides hierarchy construction and Treemap layout.
`merman-render` binds typography and writes the SVG.

The `diagram-venn` feature provides the Venn solver and typed layout. `merman-render` binds the
theme and supplies the operation's work meter and SVG writer.

The `diagram-xychart` feature provides typed XY Chart layout, plot geometry, and bar label
placement. `merman-render` supplies text measurement and writes the SVG.

Use `merman` for source-to-output rendering.
