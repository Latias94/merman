# HPD-050 - Architecture Cytoscape Label Extension

Date: 2026-06-02

## Context

The Cytoscape bbox phase split showed that Architecture should not use one global text/group bbox
formula. A smaller seam was still worth fixing: the FCoSE `BoundsExtras` path and the SVG
root/group-bounds path both recalculated the same Cytoscape service-label half-width and compound
label bottom rule.

## Outcome

- Added `ArchitectureCytoscapeServiceLabelExtension` in `architecture_metrics.rs`.
- Routed both `architecture_measure_cytoscape_node_bbox_extras(...)` and
  `architecture_estimate_service_bounds(...)` through the same extension helper for:
  - Cytoscape canvas label half-width,
  - applied label-width scale,
  - compound-label bottom extension.
- Kept SVG root `createText(...)` bbox logic separate from Cytoscape compound-child label logic.
  These are different phases and should not share the same helper.
- Added a focused unit test for the shared extension and empty-title behavior.

## Verification

- `cargo fmt --all`
- `cargo test -p merman-render architecture_cytoscape_service_label_extension_centralizes_compound_label_phase --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_node_bbox_extras_convert_to_manatee_bounds_extras --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_cy_label_extension.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_cy_label_extension.md`

## Evidence

- Architecture structural parity remained green.
- Architecture parity-root remained the expected 26 mismatches.
- The top root residual rows stayed in the known order:
  `junction_fork_join_026`, `batch5_long_titles_and_punct_076`, `html_titles_and_escapes_041`,
  and `batch6_init_fontsize_icon_size_wrap_093`.

## Notes

This is a behavior-preserving seam cleanup for normal Architecture inputs. It deliberately avoids
applying the rejected global `ceil(canvas)+labelBounds/group-padding` formula.
