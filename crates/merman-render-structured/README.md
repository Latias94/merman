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

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
