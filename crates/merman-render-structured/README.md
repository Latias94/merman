# merman-render-structured

Layout and SVG emission for structured Mermaid diagrams. The `diagram-info` feature provides typed
layout, presentation binding, and complete Info SVG emission. `merman-render` supplies the root
context and applies canvas and terminal output policies.

The `diagram-packet` feature provides typed layout, presentation binding, and complete Packet SVG
emission. `merman-render` supplies the root context and applies canvas and terminal output policies.

The `diagram-railroad` feature provides typed layout, presentation binding, and complete Railroad
SVG emission. `merman-render` supplies the operation's text measurer, theme selection, and root
context, then applies canvas and terminal output policies.

The `diagram-ishikawa` feature provides typed layout, presentation binding, and complete classic
and hand-drawn SVG emission. `merman-render` supplies the operation's text measurer, theme
selection, and root context, then applies canvas and terminal output policies.

The `diagram-cynefin` feature provides typed layout, presentation binding, and complete SVG
emission. Measurement, viewport, and work limits come from the rendering operation.

The `diagram-timeline` feature provides typed layout and complete SVG for horizontal and vertical
Timeline diagrams. The rendering operation supplies its measurement, viewport, and work policies.

The `diagram-gantt` feature provides time and task layout, presentation binding, and complete
Gantt SVG emission. The rendering operation supplies the text measurer, clock, time-zone resolver,
viewport, and work meter.

The `diagram-journey` feature provides typed layout, presentation binding, and complete Journey
SVG emission. The rendering operation supplies text measurement, viewport, and work limits.

The `diagram-tree-view` feature provides tree geometry, native and Mermaid styling, and complete
Tree View SVG emission. The render operation supplies measurement, icon, viewport, and resource
policies.

The `diagram-event-modeling` feature provides frame and swimlane layout, presentation binding,
prepared XHTML labels, and complete Event Modeling SVG emission. The rendering operation supplies
text measurement, the root viewport, and the shared work meter.

The `diagram-kanban` feature provides section and card layout, prepared XHTML labels and ticket
links, and complete Kanban SVG emission. The render operation supplies measurement, the admitted
fit width, the root viewport, and the shared work meter.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
