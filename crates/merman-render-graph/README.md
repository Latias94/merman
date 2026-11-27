# merman-render-graph

Rendering primitives for Mermaid graph diagrams. The `diagram-er` feature provides ER relationship
marker definitions and their SVG references. `layout-shoal` adds marker footprints for routing and
viewport geometry.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
