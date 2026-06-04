# HPD-050 - Architecture FCoSE Strict Rectangle Intersects

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

After the multiline group-title root-bounds fix, fresh focused evidence showed
`stress_architecture_group_port_edges_017` was again an active current residual:
upstream/local max-width was `707.769226px` / `709.237549px`, and the local root height was
`17.845154px` shorter.

That conflicted with the older post-Procrustes evidence that had made this row exact. The current
delta and force debug showed the same second-run compound-repulsion branch from the earlier audit:
`inner` was receiving overlap-style `rep=(40,40)` instead of the upstream clipping-path
`rep=(0,250)`.

## Outcome

- Restored `rects_intersect(...)` to source-strict layout-base semantics: touching edges intersect,
  but any positive gap remains non-intersecting.
- Kept the `GEOMETRY_EPSILON` path for near-equal center/direction comparisons, which preserves
  the `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` closure.
- Updated the FCoSE tests so `rects_intersect_keeps_positive_touch_gap_separate` locks the
  source-strict `RectangleD.intersects(...)` boundary, and
  `constraint_handler_preserves_group_port_second_run_tiny_gap` now also asserts that the
  positive-gap `out1`/`inner` pair takes the non-overlap clipping branch.
- This is not a root pin, fixture special case, global padding change, or solver rewrite.

## Evidence

- Focused `stress_architecture_group_port_edges_017` parity-root now passes with upstream/local
  `max-width: 707.769px`.
- Focused `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` parity-root still
  passes; its residual remains below DOM mismatch precision.
- Full Architecture parity-root remains an expected diagnostic failure, but the queue improves from
  the post-095 `23` mismatch rows to `20` mismatch rows. `group_port_edges_017` is root-exact in
  the root-delta table, and `087` remains outside the mismatch list.
- Full Architecture structural DOM parity stays green, and full all-diagram structural parity stays
  green.

## Verification

- `cargo nextest run -p manatee -E 'test(rects_intersect_keeps_positive_touch_gap_separate) or test(overlap_separation_treats_nearly_equal_centers_as_equal) or test(constraint_handler_preserves_group_port_second_run_tiny_gap)'` -
  passed, `3` tests run.
- `cargo nextest run -p manatee fcose` - passed, `10` tests run.
- `cargo nextest run -p manatee` - passed, `14` tests run.
- `cargo nextest run -p merman-render architecture` - passed, `31` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_group_port_edges_017 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-017-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-087-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture-report-parity-strict-intersect-final` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-strict-intersect-final` -
  expected-failed with `20` mismatch rows.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual Boundary

Do not reintroduce a global `rects_intersect(...)` epsilon to chase other root tails. The source
boundary is strict positive-gap handling for rectangle intersection plus narrowly tested
near-equal-center handling for overlap direction. Remaining Architecture residuals are still
service child contribution / Cytoscape bbox phase and SVG root-consumption tails unless fresh
source evidence proves a new FCoSE phase rule.
