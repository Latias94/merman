# Native Theme Coverage

Run `merman-cli theme list --json` and read `families[].capabilities` for the installed build. SDKs expose the same data through the `theme-catalog` metadata operation. An SVG-enabled full build has 34 logical families and 37 syntax entry points, including the Railroad dialects. [Renderer registry](../../crates/merman-render/src/theme/mod.rs) · [Capability flags](../../crates/merman-theme-contract/src/v2/mod.rs)

Each renderer family maps semantic settings to visible objects. A connector color applies to Flowchart arrows, Sequence messages, and XY Chart axes. Categorical slots supply the colors of series, slices, or branches where a family consumes them. The root canvas is shared. Families apply typography during layout when it affects geometry.

For charts, inspect the plotted series, labels, and legend together. XY Chart consumes gridlines, bar radius and wireframe, line point markers, and categorical patterns. Pie uses a canvas-colored gap and the series' paired text color. Sankey and Treemap draw categorical fills in their own layers. Tree View and Railroad use the code font slot for their notation.

The native capability mask describes the implemented settings for the family. Ordinary renders report theme compilation notices and an explicit source-theme selection. A request using `RequirePortable` also enforces support for requested treatments. `form.chart` applies to chart families and the Wardley grid. Elements such as a note, title, or legend still depend on the source. Test each output format with the intended fonts, background, and export options. The ASCII palette projects the family's effective roles into a terminal's color encoding.

With Shoal, Flowchart and Agentflow use native density and share the default connector style between placement and SVG. An explicit Mermaid curve or source edge default takes precedence. Swimlane uses native density with orthogonal lanes. Architecture passes native marker dimensions and shape contours to canonical routing. Across other adapters, marker clearance, endpoint stubs, and outline clipping still follow each family's existing geometry.

[Theme authoring and selection](custom-diagram-themes.md)
