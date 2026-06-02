# Headless Parity Deepening - Handoff

Status: Active
Last updated: 2026-06-02

This workstream opens the post-11.15 structural-parity phase.

Current priority order:

1. HPD-050 Architecture-first layout engine audit
2. HPD-060 semantic/render unification pilot
3. HPD-070 unsupported-family rubric

Immediate next task:

- HPD-010, HPD-020, HPD-030, and HPD-040 are done.
- Next executable slice should be HPD-050 Architecture-first layout engine audit. HPD-040 created
  the first shared measurement/root-bounds seams, so the next leverage point is to audit
  Architecture residuals through Mermaid source-backed input and bounds-feeding evidence.

Current repository reality to preserve:

- Structural `parity` is green for the implemented matrix.
- `parity-root` remains the active residual front.
- Honest top residual buckets are currently Flowchart `61`, Architecture `26`, Sequence `27`,
  Class `12`, Timeline `3`, Journey `2`.
- Sequence left-of wrapped note width semantics were improved in commit `cd9f02ff`, but a small
  root-width residual remains and should not be overfit without stronger evidence.
- Architecture remains the highest-value `manatee` / input-model audit target.
- This lane is not a license to drive every residual to zero with constants. Its purpose is to
  improve baseline truth, residual governance, and shared seams so later fixes are explainable.
- HPD-020 outcome to preserve:
  - `crates/merman-core/src/baseline.rs` owns the pinned Mermaid tag/version plus the explicit
    legacy generated suffix.
  - `Engine::default()` and live registry constructors now use
    `for_pinned_mermaid_baseline()` / `pinned_mermaid_baseline_*`.
  - xtask importers, bench entrypoints, and baseline report labels no longer present
    `default_mermaid_11_12_2*` as the active baseline truth.
  - Historical generated filenames still carry `11_12_2`; that is now explicit legacy provenance,
    not implied active baseline truth.
- HPD-030 outcome to preserve:
  - residual governance now uses six categories: source-backed behavior gap, generated measurement
    gap, browser lattice tail, stale baseline/override, solver/phase residual, and scope boundary.
  - Flowchart is currently dominated by browser lattice tails; Architecture is the main
    solver/phase front; Sequence and Class are the main generated-measurement fronts.
  - Counts remain queue-shaping hints only. They are not progress percentages and should not be
    used to justify fake completion claims.
- HPD-040 outcome to preserve:
  - `svg_emitted_bounds` is now `svg/parity` infrastructure, not State-owned code.
  - Sequence note final wrap/measure logic is centralized and reused by layout, root-bounds, and
    SVG rendering.
  - No new override growth or ad hoc constants were introduced.
  - The Sequence long leftOf note root-width residual remains open (`570px` deterministic local vs.
    `566px` upstream; headless vendored report `585px` vs `566px`) and should not be overfit.
- HPD-050 in-progress outcome:
  - Architecture's FCoSE node `BoundsExtras` adapter is now a named helper
    (`architecture_fcose_node_bounds_extras`) with direct unit coverage.
  - The layout view no longer carries group title state, matching the current source-backed rule
    that group titles do not affect the pre-layout `eles.boundingBox()` relocation center.
  - The focused batch5 long-title residual stayed unchanged at upstream `542.926px` vs local
    `547.926px`; this pass was boundary cleanup, not a hidden root-width tune.
  - A second source-backed bounds slice fixed Architecture edge-label root bounds:
    `createText()` local y-range is now transformed for X/Y edge labels instead of being treated as
    a centered AABB, and compound label bottom now uses the source-backed `fontSize + 1px` rule.
  - This made `stress_architecture_batch4_init_small_icons_061`,
    `stress_architecture_batch4_init_fontsize_wrap_063`, and
    `stress_architecture_edge_label_corner_cases_012` root-green without adding root overrides.
  - Full Architecture structural parity is still green; Architecture `parity-root` now has `26`
    mismatches. The remaining top tails are still `junction_fork_join_026` (`+13.976px`),
    `batch5_long_titles_and_punct_076` (`+5.000px`), and `html_titles_and_escapes_041`
    (`+5.000px`).
  - A follow-up `junction_fork_join_026` audit found no new source-input mismatch. Current local
    service positions match the saved Mermaid browser probe
    `target/compare/arch_junction_fork_join_probe_m15rv089.json` to floating-point noise, while the
    stored upstream SVG differs by about `7-10px` in X and `6-12px` in Y on service positions.
    Treat this row as a generated-baseline / seed-lattice audit candidate before touching manatee.
  - A follow-up bounds-seam cleanup removed the unused renderer-side `initial_center` / pre-layout
    group bbox model and renamed the old generic compound padding helper to
    `architecture_svg_group_bbox_padding_px(...)`. The remaining `batch5_long_titles` and
    `html_titles` `+5px` tails are confirmed group/service Cytoscape bbox measurement residuals;
    do not close them by globally removing the final SVG group bbox extra.
