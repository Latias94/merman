# merman-render-charts

The `diagram-pie` feature provides typed layout, theme binding, and SVG content for Pie diagrams.
`merman-render` supplies the operation's text measurement, root viewport, and SVG output policy.

The `diagram-quadrant-chart` feature provides typed geometry, paint binding, and SVG content
for Quadrant Chart. `merman-render` resolves theme precedence and supplies the SVG root,
accessibility, and stylesheet.

The `diagram-radar` feature provides typed layout and the point and radius codecs for Radar.
`merman-render` resolves the theme and writes the SVG.

The `diagram-sankey` feature provides typed layout, theme binding, label measurement, and complete
SVG emission. `merman-render` supplies the operation's measurement and viewport context and
applies the terminal SVG policies.

The `diagram-treemap` feature provides layout, theme binding, label fitting, and complete SVG
emission. `merman-render` supplies the operation's text measurement and viewport context and
applies the terminal SVG policies.

The `diagram-venn` feature provides the Venn solver and typed layout. `merman-render` binds the
theme and supplies the operation's work meter and SVG writer.

The `diagram-wardley` feature provides typed layout, geometry projection, and annotation box
sizing. `merman-render` resolves the theme, measures annotation labels, and writes the SVG.

The `diagram-xychart` feature provides typed XY Chart layout, plot geometry, and bar label
placement. `merman-render` supplies text measurement and writes the SVG.

Use `merman` for source-to-output rendering.
