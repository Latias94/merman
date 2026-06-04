# HPD-050 - Architecture Edge Curve Style Relocate Fix

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

The relocate center diagnostics narrowed `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`
to a run 1 `eles.boundingBox()` input-model drift. Browser render-path evidence showed the key
`api-db` edge was still `curve-style: straight` at
`layoutstop-run1-after-segments-before-run2`, even though Mermaid had written
`segment-weights` / `segment-distances` after run 0. Local `manatee` treated every diagonal edge
as a post-run segmented orthogonal edge and placed the edge-label bbox at the bend point.

## Change

- Added `IndexedEdge::curve_style_segments` so FCoSE bbox relocation can distinguish real
  Cytoscape `edge.segments` edges from ordinary diagonal `straight` edges.
- Architecture now sets that flag from the Mermaid direction-pair rule: one horizontal endpoint
  direction and one vertical endpoint direction means `edge.segments`; same-axis edges remain
  `straight`.
- `bounding_box_center_eles(...)` now applies pre-run and post-run segment control-point label
  semantics only when `curve_style_segments` is true.
- Architecture FCoSE edge-label measurement now uses Cytoscape's default edge label text style:
  Mermaid Architecture sets `font-size` on `node[label]`, not on `edge[label]`.

## Findings

- Before this fix, local `093` run 1 put `api-db`'s label center at the orthogonal bend:
  `x=82.037349 y=-66.880297`, pushing the local `eles.boundingBox()` right edge too far.
- After this fix, the same label center is `x=40.000000 y=12.407124`, matching the browser's
  straight-edge midpoint semantics.
- The bundled/local run 1 relocate `originalCenter.x` drift for `093` dropped from
  `+22.963987px` to `+1.230469px`.
- `002` remains at the known `+1.250000px` relocate origin drift.
- Both `002` and `093` still have a `2.5px` root-width residual. This change fixes the
  source-backed edge curve-style input model; it does not close the remaining final group/root
  bounds tail.

## Evidence

- `target/compare/architecture-delta-segment-style-fix-hpd050`
- `target/compare/architecture-report-parity-segment-style-fix-hpd050`
- `target/compare/architecture-report-parity-root-segment-style-fix-hpd050`

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p manatee` - passed, `15` tests run.
- `cargo nextest run -p merman-render architecture` - passed, `32` tests run.
- `cargo nextest run -p xtask` - passed, `112` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-segment-style-fix-hpd050` -
  passed.
- `MANATEE_FCOSE_DEBUG_TRACE=1 MANATEE_FCOSE_DEBUG_ELES_BBOX=1 cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_nested_groups_002 --fixture stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --out target\compare\architecture-delta-segment-style-fix-hpd050 --render-probe-dir target\compare\architecture-render-path-source-frame-002-093-hpd050` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-segment-style-fix-hpd050` -
  expected-failed with the active `20` Architecture root mismatch rows.

## Boundary

Do not treat this as a group padding, root padding, or final rect emission fix. The next source
seam is the remaining final group/root bbox tail: `093` now reports direct group-width deltas
(`left=-3px`, `right=-1px`) and `002` remains a nested frame/root-width sensor.
