# merman-render-graph

Rendering for Mermaid graph diagrams. The `diagram-architecture` feature owns Architecture
presentation and complete SVG emission. `layout-cytoscape` adds the fCoSE adapter. The
facade selects the backend and supplies the operation context and canonical routes.

The `diagram-er` feature owns ER configuration, theme
binding, label preparation, and complete SVG emission. `merman-render` selects the layout backend,
supplies canonical routes and the operation context, and applies terminal SVG policies.
`layout-dagre` enables the ER Dagre adapter. `layout-shoal` adds marker footprints for routing
and viewport geometry.

The `diagram-class` feature owns Class style binding, label preparation, and complete SVG
emission. The facade selects the layout backend and supplies measured geometry and canonical
routes. `layout-dagre` enables the Class Dagre adapter.

The `diagram-c4` feature owns C4 style and text preparation, grid layout, and complete SVG
emission. The facade supplies routed geometry when the operation selects Shoal.

The `diagram-requirement` feature owns Requirement preparation, Dagre layout, ELK graph
projections, and complete SVG emission. `merman-render` selects the backend and supplies
canonical geometry, viewport settings, and terminal policies.

The `diagram-git-graph` feature owns GitGraph preparation, layout, theme binding, and
complete SVG emission. `merman-render` supplies routed measurement, viewport context,
and terminal SVG policies.

The `diagram-mindmap` feature owns Mindmap styles, prepared labels, TidyTree layout, and
complete SVG emission. Optional loader features enable Dagre, ELK, or CoSE-Bilkent projections.
`merman-render` selects the loader, supplies operation measurement and shared ELK and Swimlane
services, and applies terminal SVG policies.

The `diagram-state` feature owns State presentation binding, label measurement, and
complete SVG emission. `merman-render` selects the layout backend, supplies its route
geometry and the operation context, and applies terminal SVG policies. `layout-dagre` enables
the State Dagre adapter.

The `diagram-usecase` feature owns Usecase style binding, label preparation, Dagre layout,
and complete SVG emission. `merman-render` selects the layout backend, supplies canonical
routes when using Shoal, and applies terminal SVG policies.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
