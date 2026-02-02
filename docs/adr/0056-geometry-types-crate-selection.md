# ADR-0056: Geometry Types Crate Selection (Points/Rects/Transforms)

## Status

Accepted

## Context

`merman` targets full SVG DOM parity with Mermaid `@11.12.2`. Across parsing, layout, and rendering,
we routinely need:

- 2D points/vectors (node positions, label anchors)
- sizes/rects (bbox, padding, cluster/subgraph bounds)
- 2D transforms (translate/scale/rotate; text and marker placement)

Today, several modules define ad-hoc `Point`/`Rect` structs, which increases the risk of:

- inconsistent semantics (min/max vs center/size)
- inconsistent numeric helpers (trig, angles, transforms)
- extra conversion glue when integrating other geometry-heavy crates

We also already depend on a geometry crate transitively via `roughr` (used for the hand-drawn style).

## Decision

Use `euclid` as the workspace-standard crate for **2D geometry primitives**:

- Add `euclid` to `[workspace.dependencies]`.
- Introduce `merman-core::geom` as the canonical place to re-export geometry type aliases:
  - `Point`, `Vector`, `Size`, `Rect`, `Transform`
- Expose a min/max bounding-box helper `merman-core::geom::Box2` (a thin wrapper over
  `euclid::Box2D`) for code that naturally operates on `min/max` extents (clusters, subgraphs, bbox
  unions).
- Prefer `f64` as the coordinate scalar to match Mermaid's JS `Number` behavior and to reduce
  parity drift due to rounding.

Keep `nalgebra` for **dense linear algebra** only (SVD/eigen/power iteration) as per ADR-0054.

## Alternatives Considered

1. `glam`
   - Pros: excellent SIMD-friendly vector/matrix types; popular in rendering/game ecosystems.
   - Cons: no rect/size/unit-typed geometry; would still require another crate or custom types; does
     not help with the SVD/eigen requirements of layout ports.

2. Hand-rolled geometry structs
   - Pros: minimal dependencies; full control over API surface.
   - Cons: long-term maintenance cost; easy to diverge in semantics across modules; re-implements
     transforms/trig/rect helpers that are already well-tested elsewhere.

3. `kurbo` (2D curves/path geometry)
   - Pros: strong path primitives and curve math.
   - Cons: not primarily focused on unit-typed points/rects; better evaluated when we reach path
     and stroke parity work.

## Consequences

- Pros:
  - A single, well-defined geometry vocabulary across crates.
  - Reduced conversion friction with `roughr` (already uses `euclid` points).
  - Clear separation between geometry (`euclid`) and dense linear algebra (`nalgebra`).
- Cons:
  - Requires gradual migration from ad-hoc structs to `merman-core::geom` aliases.
  - Adds a direct workspace dependency (though already effectively present via `roughr`).
