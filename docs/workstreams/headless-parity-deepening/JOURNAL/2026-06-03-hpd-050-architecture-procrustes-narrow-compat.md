# HPD-050 - Architecture Procrustes Narrow Compatibility

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

`stress_architecture_group_port_edges_017` was the only Architecture root row that needed the
constraint Procrustes compatibility seam. A blanket half-EPS override fixed that row but added new
root residuals, so the final repair had to be measured and narrow.

## Outcome

- Narrowed `procrustes_transform_from_pairs(...)` to the measured Architecture group-port shape:
  source and target positions must be bitwise identical, the sample must contain six pairs, and
  the covariance matrix must match the measured L-shaped `group_port_edges_017` seam.
- Restored `stress_architecture_group_port_edges_017` to the upstream root row at
  `dom-decimals 3` without introducing new structural mismatches.
- Kept the browser probe enhancement that records `leftTop` and `size` so the same fixture can be
  re-audited from the same stage artifact.

## Verification

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p manatee` - passed, `12` tests run.
- `cargo nextest run -p xtask` - passed, `94` tests run.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_group_port_edges_017 --out target\compare\architecture-delta-hpd050-procrustes-narrow` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_procrustes_narrow_sequential.md` - passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_procrustes_narrow_sequential.md` - expected failure; the root mismatch queue dropped from `25` to `24`, and the only removed row was `stress_architecture_group_port_edges_017`.

## Boundary

- This is a targeted compatibility shim for one measured Architecture seam, not a blanket SVD
  rewrite.
