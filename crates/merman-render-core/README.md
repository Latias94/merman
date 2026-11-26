# merman-render-core

Shared layout coordinates and SVG edge geometry for `merman-render`. Default builds provide
layout and route types. `route-paint` enables SVG path construction with marker setback and
obstacle clearance. `graph-layout` provides geometry for measured and solved graphs.
`layout-shoal` shares the solver's route types. `shoal-bridge` uses the same types to project
measured diagrams into the solver and solved geometry back into the renderer. The `resources`
feature provides render policies and the operation meter that shares limits and cancellation
across layout and SVG emission. The renderer supplies that meter to the Dagre, ELK, Shoal, and
Cytoscape adapters selected by its layout features.
`svg-output` provides bounded SVG writing, XML text serialization, base styles, and shared render
errors. `svg-parse` provides lexical scanning, XML stylesheet decoding, and SVG structure
validation for postprocessing.
`svg-animation` validates and removes Mermaid Step animations for static SVG output.
`svg-identity` rewrites Mermaid marker selectors for the native SVG CSS engine.
`css-style` provides presentation lookups, Mermaid CSS value parsing, and shared text style and
measurement types. `text-layout` provides font stacks, text tokenization, line breaking, and width
estimation. `theme-svg` writes resolved canvas paint, markers, and categorical patterns to SVG.
`theme-elevation` and `theme-label-background` add shadow silhouettes and measured label
backplates, respectively, for the families that use them.
`icon-registry` admits and stores Iconify packs under fixed resource limits. The renderer
applies its SVG output policy when an icon is placed in a diagram.

This crate provides workspace-internal renderer interfaces. Applications use `merman` for the
public source-to-output API.
