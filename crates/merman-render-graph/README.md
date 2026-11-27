# merman-render-graph

Rendering for Mermaid graph diagrams. The `diagram-er` feature owns ER configuration, theme
binding, authored styles, the SVG stylesheet, and relationship markers. `merman-render` owns
backend selection, label preparation, and SVG document emission. `layout-shoal` adds marker
footprints for routing and viewport geometry.

The `diagram-git-graph` feature owns GitGraph preparation, layout, theme binding, and
complete SVG emission. `merman-render` supplies routed measurement, viewport context,
and terminal SVG policies.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
