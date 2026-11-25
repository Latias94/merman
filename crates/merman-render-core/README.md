# merman-render-core

Shared layout coordinates and SVG edge geometry for `merman-render`. Default builds provide
layout and route types. `route-paint` enables SVG path construction with marker setback and
obstacle clearance. `layout-shoal` uses Shoal's route types directly so the renderer can pass
the solved route points to the path builder. The `resources` feature provides render policies and
the operation meter that shares limits and cancellation across layout and SVG emission.
`svg-output` provides the bounded SVG writer, XML text serialization, base styles, and shared
render errors. `css-style` provides presentation lookups, Mermaid CSS value parsing, and shared
text style and measurement types. `theme-svg` writes resolved canvas paint and categorical
patterns to SVG.

Applications use `merman` for source-to-output rendering.
