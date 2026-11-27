# merman-render-charts

The `diagram-pie` feature provides preparation, typed layout, theme binding, and complete SVG
emission for Pie diagrams. `merman-render` supplies the operation's text measurer and viewport
context and applies the terminal SVG policies.

The `diagram-quadrant-chart` feature provides preparation, typed geometry, paint binding, and
complete SVG emission. `merman-render` supplies the selected theme and viewport context and
applies the terminal SVG policies.

The `diagram-radar` feature provides preparation, typed layout, theme binding, and complete SVG
emission for Radar. `merman-render` supplies the selected theme, viewport context, and final SVG
policies.

The `diagram-sankey` feature provides typed layout, theme binding, label measurement, and complete
SVG emission. `merman-render` supplies the operation's measurement and viewport context and
applies the terminal SVG policies.

The `diagram-treemap` feature provides layout, theme binding, label fitting, and complete SVG
emission. `merman-render` supplies the operation's text measurement and viewport context and
applies the terminal SVG policies.

The `diagram-venn` feature provides the Venn solver, typed preparation, theme binding, and
complete classic and hand-drawn SVG emission. `merman-render` supplies the selected theme,
viewport, and operation context and applies the terminal SVG policies.

The `diagram-wardley` feature provides preparation, typed layout, annotation measurement, theme
binding, and complete SVG emission. `merman-render` supplies the selected theme, viewport,
measurement, and canvas context and applies the terminal SVG policies.

The `diagram-xychart` feature provides typed XY Chart layout, plot geometry, and bar label
placement. `merman-render` supplies text measurement and writes the SVG.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
