# HPD-050 - Architecture Child Source Phase Experiments

Date: 2026-06-03

## Context

The children-bbox probe confirmed the upstream Cytoscape source formula:

- parent `autoWidth` / `autoHeight` come from
  `children.boundingBox({ includeLabels: true, includeOverlays: false, useCache: false })`;
- child service `labelBounds.w` is browser canvas `labelWidth + 4`;
- child service `bodyBounds` is the node body expanded by Cytoscape's `+1px` browser inaccuracy
  margin;
- final parent `node.boundingBox()` adds the later outer/padding/body-expansion phase.

This pass tested whether those source phases can replace the current headless approximation in
production.

## Baseline

Current HEAD baseline:

- Architecture `parity-root`: `25` DOM mismatches.
- Focused `batch5_long_titles_and_punct_076`: upstream `542.926`, local `547.926`, delta `+5.000`.
- Focused `html_titles_and_escapes_041`: upstream `479.926`, local `484.926`, delta `+5.000`.

## Experiment A - labelBounds source formula only

Temporary production patch:

- added a source-formula label half-width:
  `ceil(headless_measured_width) / 2 + 2`;
- used that value only for `cytoscape_group_child_bounds`;
- kept existing body bounds (`80px` icon body) and final group padding
  (`padding + 2.5px`).

Results:

- focused `batch5`: delta improved from `+5.000` to `+4.500`;
- focused `html_titles`: delta improved from `+5.000` to `+3.500`;
- full Architecture `parity-root` DOM mismatches improved from `25` to `24`.

Rejection reason:

- this is only a half-source model: labelBounds uses the source formula, but body/final group phases
  remain the old compensation model;
- it worsened some already-small residuals, e.g. `batch3_long_group_titles_wrapping_055` shifted
  from `-1.000` to `-2.500`, and `long_labels_006` shifted from `-0.500` to `-1.500`;
- it does not solve the phase model and would make later source-backed body/final-group work harder
  to reason about.

## Experiment B - child body, child label, and final group source phase

Temporary production patch:

- child body bounds start from `iconSize` expanded by Cytoscape's `+1px` body-bounds margin;
- child label bounds use `ceil(headless_measured_width) / 2 + 2`;
- final group padding uses `padding + 1.5px`, matching child auto bounds plus border/body expansion
  rather than the current mixed compensation.

Results:

- focused `batch5`: delta improved from `+5.000` to `+2.500`;
- focused `html_titles`: delta improved from `+5.000` to `+1.500`;
- Architecture structural `parity` stayed green;
- full Architecture `parity-root` DOM mismatches expanded from `25` to `100`.

Rejection reason:

- source phase structure is right, but our current headless text/body/group approximation is not yet
  broad enough for a direct replacement;
- many group-heavy and nested fixtures became too small, e.g. `deep_group_chain_027` `-7.000`,
  `batch6_deep_group_chain_crosslinks_094` `-6.000`, and
  `batch6_nested_groups_group_edges_and_ports_086` `-5.000`;
- this would regress previously classified or green rows and violates the workstream rule against
  local root parity wins that damage the family.

Both production patches were reverted before commit.

## Outcome

Do not currently replace `cytoscape_group_child_bounds` with raw Cytoscape source formulas. The
right next seam is broader than a group padding or label half-width helper:

1. keep separate models for SVG `createText(...)`, FCoSE node `BoundsExtras`, Cytoscape
   child-label bounds, child-body bounds, and final parent `node.boundingBox()`;
2. improve or calibrate the headless text measurement seam before using source labelBounds as a
   production replacement;
3. validate any future phase helper against nested/group-heavy fixtures first, not only the two
   `+5px` rows.

## Verification

- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_child_bounds_baseline.md`
- `cargo nextest run -p merman-render architecture_cytoscape_label_bounds_half_width_matches_source_formula architecture_cytoscape_service_label_extension_centralizes_compound_label_phase architecture_text_constants_match_mermaid`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_child_label_bounds_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_child_label_bounds_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_child_label_bounds_experiment.md`
- `cargo nextest run -p merman-render architecture_cytoscape_label_bounds_half_width_matches_source_formula architecture_cytoscape_service_label_extension_centralizes_compound_label_phase architecture_text_constants_match_mermaid architecture_svg_group_bbox_padding_adds_headless_cytoscape_extra`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_child_source_phase_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_child_source_phase_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_hpd050_child_source_phase_experiment.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_hpd050_child_source_phase_experiment.md`
