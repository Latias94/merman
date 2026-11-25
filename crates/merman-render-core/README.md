# merman-render-core

Shared layout coordinates and SVG edge geometry for `merman-render`. Default builds provide
layout and route types. `route-paint` enables SVG path construction with marker setback and
obstacle clearance. `layout-shoal` uses Shoal's route types directly so the renderer can pass
the solved route points to the path builder. The `resources` feature provides render policies and
the operation meter that shares limits and cancellation across layout and SVG emission. The
renderer supplies that meter to the Dagre, ELK, Shoal, and Cytoscape adapters selected by its
layout features.
`svg-output` provides bounded SVG writing, XML text serialization, base styles, and shared render
errors. `svg-parse` provides lexical scanning, XML stylesheet decoding, and SVG structure
validation for postprocessing.
`svg-animation` validates and removes Mermaid Step animations for static SVG output.
`svg-identity` rewrites Mermaid marker selectors for the native SVG CSS engine.
`css-style` provides presentation lookups, Mermaid CSS value parsing, and shared text style and
measurement types. `text-layout` provides font stacks, text tokenization, line breaking, and width
estimation. `theme-svg` writes resolved canvas paint and categorical patterns to SVG.
`icon-registry` admits and stores Iconify packs under fixed resource limits. The renderer
applies its SVG output policy when an icon is placed in a diagram.

Applications use `merman` for source-to-output rendering.
