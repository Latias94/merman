# merman-render-core

Shared layout coordinates and SVG edge geometry for `merman-render`. Default builds provide
layout and route types. `route-paint` enables SVG path construction with marker setback and
obstacle clearance. `graph-layout` provides geometry for measured and solved graphs.
`layout-shoal` shares the solver's route types. `shoal-bridge` uses the same types to project
measured diagrams into the solver and solved geometry back into the renderer. The `resources`
feature provides render policies, shared render errors, and the operation meter that shares
limits and cancellation across layout and SVG emission. The `layout-dagre`, `layout-elk`, and
`layout-cytoscape` features each use that meter without requiring an SVG writer. The `shoal-bridge`
feature provides the metered adapter for Shoal.
`svg-output` provides bounded SVG writing, XML text serialization, base styles, and the output
and compatibility error variants. `root-svg` provides viewport planning and emission for family
SVG documents. The SVG facade uses `error-diagram` for its always-available Error diagram's
typography, viewport, and complete SVG writer.
`layout-elk` provides shared Mermaid ELK hierarchy, group frames, and endpoint geometry. With
`graph-layout`, it also provides missing-section and terminal-jog projection. With `css-style`,
it provides ELK option projection and feedback edge handling. The geometry helpers also remain
available with `svg-output` for families that consume routes without the solver.
`paint-scene` provides an internal typed scene and bounded SVG writer for resolved geometry and
paint. The writer does not select an output profile or replace terminal SVG admission.
`svg-parse` provides lexical scanning, XML stylesheet decoding, and SVG structure
validation for postprocessing. `svg-safety` provides the bounded CSS and attribute sanitizers
and the inline raster URL policy used by terminal SVG validation.
`svg-animation` validates and removes Mermaid Step animations for static SVG output.
`svg-bounds` measures emitted SVG shape and path bounds for parity viewports.
`svg-identity` rewrites Mermaid marker selectors for the native SVG CSS engine.
`css-style` provides presentation lookups, Mermaid CSS value parsing, and the shared `TextMeasurer`
trait and its value types. `text-layout` provides font stacks, deterministic measurement, and
line breaking. With `resources`, it also provides host measurement routing and provenance for one
operation. `theme-svg` writes resolved canvas paint, markers, and categorical patterns to SVG.
`xhtml-labels` provides XHTML label font measurement and plain-paragraph projection.
`markdown-labels` adds Markdown source words, bounded HTML projection, and shared wrapping for
HTML and SVG label terminals.
`math-labels` provides shared label measurement and prepared math projections. The optional `math`
feature adds the RaTeX backend and embedded math fonts. Family renderers share the bounded HTML
sink and SVG navigation policy from `html`. The facade owns whole-document math reference checks.
`theme-elevation` and `theme-label-background` add shadow silhouettes and measured label
backplates, respectively, for the families that use them.
`icon-registry` admits and stores Iconify packs under fixed resource limits. The renderer
applies its SVG output policy when an icon is placed in a diagram.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Applications use `merman` for the public source-to-output API.
