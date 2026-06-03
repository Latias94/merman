# HPD-050 - Architecture Cytoscape Child Contribution Bounds

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous Architecture child-label bounds cleanup made the Cytoscape label phase explicit, but
the service bounds estimate still exposed one aggregate `cytoscape_group_child_bounds` field. The
earlier failed source-formula experiments showed why that is too coarse: future work needs to keep
body bounds, label bounds, and their union distinguishable before trying any source-backed formula
change.

## Outcome

- Added `ArchitectureCytoscapeChildContributionBounds` with `body_bounds`, optional `label_bounds`,
  and `union_bounds`.
- Removed the old single `cytoscape_group_child_bounds` field from
  `ArchitectureServiceBoundsEstimate`.
- Updated SVG/group service-bounds estimation and isolated top-level service root-bounds selection
  to consume `cytoscape_group_child_contribution.union_bounds`.
- Extended `MERMAN_ARCH_DEBUG_SERVICE_BOUNDS` output so body, label, and union phases are visible
  in future probes.
- Kept the existing measurement formulas and constants unchanged.

## Verification

- `cargo fmt --check -p merman-render` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `28` tests run.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_child_contribution.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_hpd050_child_contribution.md` -
  expected-failed with the existing `25` Architecture root-only mismatches. The leading rows remain
  `junction_fork_join_026` (`+13.976px`), `batch5_long_titles_and_punct_076` (`+5.000px`), and
  `html_titles_and_escapes_041` (`+5.000px`).
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed; implemented-matrix structural parity stayed green after this seam.

## Residual Boundary

This is a behavior-preserving phase-modeling seam. It makes the source-backed Cytoscape child
contribution easier to audit, but it does not claim Architecture root residual closure.
