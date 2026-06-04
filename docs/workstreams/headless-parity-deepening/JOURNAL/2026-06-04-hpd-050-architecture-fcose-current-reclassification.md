# HPD-050 - Architecture FCoSE Current Reclassification

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous handoff still treated `stress_architecture_junction_fork_join_026` as the active
Architecture root tail and suggested comparing local `manatee` sequencing against bundled FCoSE run
`1`. Fresh current-HEAD reports no longer support that starting point: the junction fixture is now
root-green at the current `parity-root` precision, while
`stress_architecture_batch6_junctions_multi_split_with_group_edges_087` is the active large
Architecture root residual.

## Findings

- `stress_architecture_junction_fork_join_026` now passes focused Architecture `parity-root`.
- Its remaining numeric root delta is only `-0.000244px` max-width/viewBox width, which rounds to
  `-0.000` in the report and does not produce a DOM failure at `--dom-decimals 3`.
- Full Architecture structural `parity` is still green.
- Full Architecture `parity-root` is still an expected diagnostic failure. The leading row is now
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` with `+46.001831px`
  max-width/viewBox width delta.
- The render-path delta join shows the `batch6` row is not a missing-element or stale stored SVG
  problem. Render-path stored facts match the upstream fixture, while local group/service positions
  are displaced almost symmetrically:
  - `edge` group/local service side: about `-23.000899px` on X.
  - `core` group/local service side: about `+23.000899px` on X.
  - `core` group height is also `+7.345448px` larger locally.
- This shape is not the old `junction_fork_join_026` second-rerun hypothesis. It should be treated
  as a new `batch6` group/service phase residual unless future source evidence finds a reusable
  FCoSE/Cytoscape rule.
- No production renderer, `manatee`, root override, SVG fixture, or generated baseline changed.

## Artifacts

- `target\compare\architecture_junction_current_hpd050_fcose.md`
- `target\compare\architecture_batch6_junctions_current_hpd050_fcose.md`
- `target\compare\architecture_report_parity_current_hpd050_fcose.md`
- `target\compare\architecture_report_parity_root_current_hpd050_fcose.md`
- `target\compare\architecture-render-path-current-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- `target\compare\architecture-render-path-current-hpd050\stress_architecture_batch6_junctions_multi_split_with_group_edges_087.render-path-probe.json`
- `target\compare\architecture-delta-render-path-current-hpd050\stress_architecture_junction_fork_join_026.md`
- `target\compare\architecture-delta-render-path-current-hpd050\stress_architecture_batch6_junctions_multi_split_with_group_edges_087.md`
- `target\compare\architecture-delta-render-path-current-hpd050\architecture-delta-batch.md`

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_junction_current_hpd050_fcose.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_batch6_junctions_current_hpd050_fcose.md` -
  expected-failed with the `max-width` style mismatch `653.25px` upstream vs `699.25px` local.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_current_hpd050_fcose.md` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_current_hpd050_fcose.md` -
  expected-failed with the current root/style queue led by `batch6_junctions_multi_split_with_group_edges_087`.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root overrides
  remain `0`.
- `cargo run -p xtask -- debug-architecture-delta --fixture stress_architecture_junction_fork_join_026 --fixture stress_architecture_batch6_junctions_multi_split_with_group_edges_087 --render-probe-dir target\compare\architecture-render-path-current-hpd050 --out target\compare\architecture-delta-render-path-current-hpd050` -
  passed and wrote the two focused delta reports plus `architecture-delta-batch.md`.

## Residual Boundary

This is an evidence-only classification slice. Do not tune root width, group padding, emitted group
rectangles, or `manatee` rerun sequencing for `junction_fork_join_026` from the superseded handoff.
The next production-capable Architecture slice should start from the current `batch6` group/service
offset evidence and require a source-backed family-level rule before changing code.
