# HPD-050 - Architecture FCoSE Node BoundsExtras Contribution

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous child-contribution bounds seam made SVG/root service bounds consume an explicit
body/label/union model, but the FCoSE `BoundsExtras` adapter still derived left/right/top/bottom
from a local `half_w` / `bottom` formula. That kept two equivalent but differently-shaped models in
the same file.

## Outcome

- Added an internal FCoSE node-bounds contribution helper that models:
  - expanded icon/body bounds,
  - optional child label bounds,
  - union bounds used to derive `BoundsExtras`.
- Reworked `architecture_measure_cytoscape_node_bbox_extras(...)` to derive extras from that union
  instead of maintaining a separate implicit formula.
- Extended `MERMAN_ARCH_DEBUG_CY_BBOX=1` output with body, label, and union phases.
- Kept all existing measurement constants and output behavior unchanged.

## Verification

- `cargo nextest run -p merman-render architecture` - passed, `29` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_contribution.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_fcose_contribution.md` -
  expected-failed with the existing `25` Architecture root-only mismatches.

## Residual Boundary

This is a behavior-preserving phase-modeling seam. It makes FCoSE node bounds and SVG/root service
bounds share the same child contribution vocabulary, but it does not replace the headless
measurement model or claim Architecture root residual closure.
