# merman-render-charts

The `diagram-pie` feature provides preparation, typed layout, theme binding, and complete SVG
emission for Pie diagrams. `merman-render` supplies the operation's text measurer and viewport
context and applies the terminal SVG policies.

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

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
