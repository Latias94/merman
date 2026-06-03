# HPD-050 - Architecture Child Label Bounds Seam

Date: 2026-06-03

## Context

The latest Architecture source-phase experiments showed that raw Cytoscape child body/label/final
group formulas are not safe to drop directly into production yet: the focused `+5px` rows improved,
but full Architecture root mismatches expanded from `25` to `100`.

The safe next move was therefore an ownership-boundary slice, not another numeric tune. The code
already separated SVG root `createText(...)` measurement from Cytoscape compound-child
measurement, but the shared helper was still named as a generic service-label extension.

## Outcome

- Renamed the shared Architecture Cytoscape label seam to
  `ArchitectureCytoscapeChildLabelBounds`.
- Added an explicit `bounds_for_icon(...)` helper so the child-label phase is represented as bounds
  that can be unioned with service icon bounds.
- Kept FCoSE node `BoundsExtras` and SVG/group service-bounds estimation on the same existing
  half-width and bottom-extension values.
- No production layout constants changed.
- Architecture structural parity stayed green, and the root diagnostic remained the existing
  `25` mismatch queue.

## Touched Surfaces

- `crates/merman-render/src/architecture_metrics.rs`

## Verification

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `27` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_child_bounds_seam.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_child_bounds_seam.md` -
  expected failure with the existing `25` Architecture root-only mismatches. The leading rows remain
  `junction_fork_join_026` at `+13.976px`, `batch5_long_titles_and_punct_076` at `+5.000px`, and
  `html_titles_and_escapes_041` at `+5.000px`.

## Residual Boundary

This slice makes the child-label bounds phase explicit but does not claim Architecture root
residual closure. Future production changes still need to improve or calibrate the headless
measurement model broadly enough to survive the full Architecture root suite.
