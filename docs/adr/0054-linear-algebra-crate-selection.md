# ADR-0054: Linear Algebra Crate Selection (SVD/Eigen for Layout Ports)

## Status

Accepted

## Context

`merman` aims for full SVG DOM parity with Mermaid `@11.12.2`, including diagrams whose upstream
layouts rely on Cytoscape engines:

- Architecture: `cytoscape-fcose` (spectral initialization + CoSE refinement)
- Mindmap: `cose-bilkent` (force-directed with several matrix utilities upstream)

In upstream `cytoscape-fcose`, the *spectral* stage uses:

- Column/row sampling over graph shortest-path distances
- SVD-based regularization to compute an approximate inverse for the sampled sub-matrix
- Power iteration to extract the dominant eigenvectors, which become initial node coordinates

To port this behavior faithfully in Rust (headless), we need a linear algebra library that supports:

- Dynamic matrices/vectors (graph size varies per input)
- SVD decomposition in pure Rust (or at least without mandatory native BLAS/LAPACK)
- Cross-platform build stability (Windows CI and contributor experience)

## Decision

Use `nalgebra` as the workspace-standard linear algebra dependency for layout ports.

- Add `nalgebra` to `[workspace.dependencies]`.
- Use `nalgebra::DMatrix<f64>`, `nalgebra::DVector<f64>`, and `nalgebra::linalg::SVD<f64, _>` in
  `manatee` for:
  - FCoSE spectral initialization
  - Future matrix helpers that are required to match upstream tie-breaking and numerics

Keep the dependency usage internal to `manatee` so that a future swap (if needed) does not become a
public API break.

## Alternatives Considered

1. `glam`
   - Pros: very fast SIMD-friendly vector/matrix types; popular in game/rendering ecosystems.
   - Cons: does not provide SVD/eigendecomposition needed for the upstream spectral stage.

2. `ndarray` + `ndarray-linalg`
   - Pros: strong ecosystem for numerical computing.
   - Cons: `ndarray-linalg` typically depends on native BLAS/LAPACK, which complicates builds and
     reproducibility on Windows.

3. `faer`
   - Pros: modern, high-performance pure-Rust linear algebra; good long-term option.
   - Cons: larger migration surface and different APIs; not necessary for the initial port.

4. Hand-rolled SVD/eigen solvers
   - Pros: full control and minimal dependencies.
   - Cons: high risk, hard to validate, and likely to diverge from upstream numerics.

## Consequences

- Pros:
  - Enables a faithful port of the FCoSE spectral stage (SVD + power iteration).
  - Keeps builds portable and avoids mandatory native dependencies.
  - Provides a single consistent math foundation for future layout ports.
- Cons:
  - Adds a relatively large dependency to the workspace.
  - May require careful numerical stabilization to mirror upstream JS `Number` behavior.

