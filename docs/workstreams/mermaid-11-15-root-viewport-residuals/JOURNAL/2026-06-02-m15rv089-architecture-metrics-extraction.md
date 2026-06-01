# M15RV-089 - Architecture Metrics Extraction

Date: 2026-06-02
Task: M15RV-089

## Summary

Extracted the Architecture-specific Cytoscape/browser-bbox approximation helpers out of
`crates/merman-render/src/architecture.rs` into a dedicated
`crates/merman-render/src/architecture_metrics.rs` module.

## Why

The remaining Architecture `parity-root` rows are no longer a good fit for "just keep tuning one
more constant". Recent M15RV-089 evidence repeatedly split the bucket into:

- source-backed fixes that belong in layout/FCoSE input semantics,
- browser/Cytoscape canvas-label bbox approximation tails,
- and solver/compound phase drift that should stay explicit until we have better evidence.

Keeping all of the approximation constants and helper functions inline inside the main layout file
made that boundary blurry. This extraction makes the approximation layer explicit and gives future
generated evidence or measurer replacements one place to land.

## Scope

- Moved Architecture-only constants and helpers into `architecture_metrics.rs`:
  - canvas-label width scale
  - service-label bottom extension
  - createText bbox helpers
  - compound bbox padding helper
  - Cytoscape node bbox extra approximation helper
- Kept behavior unchanged:
  - `architecture.rs` still calls the same helper for pre-layout `eles.boundingBox()`-style
    relocation input.
  - Existing SVG parity modules continue importing Architecture metrics from the crate-level
    surface.
- Moved the constants test into the new module so the extracted seam stays covered.

## Notes

This is an architecture/deepening refactor, not a residual-count claim. It does not make
Architecture closer to Mermaid by itself. The point is to reduce future self-deception:
approximation policy is now easier to inspect, replace, or document as headless-only behavior.
