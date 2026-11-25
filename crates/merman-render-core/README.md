# merman-render-core

Shared layout coordinates and SVG edge geometry for `merman-render`. Default builds provide
layout and route types. `route-paint` enables SVG path construction with marker setback and
obstacle clearance. `layout-shoal` uses Shoal's route types directly so the renderer can pass
the solved route points to the path builder.

Applications use `merman` for source-to-output rendering.
