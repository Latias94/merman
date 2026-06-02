# HPD-050 Service Bounds Phase Names

Date: 2026-06-02

## Change

Renamed Architecture service bounds fields so the renderer no longer treats three different bbox
phases as generic `icon/root/compound` data:

- `emitted_icon_bounds`: the actual icon bounds emitted into SVG
- `svg_root_bounds`: the top-level service bounds used to approximate final SVG `getBBox()`
- `cytoscape_group_child_bounds`: the child bounds used when final group rectangles include service
  labels

This is intentionally behavior-preserving. It records the phase split discovered by the Cytoscape
bbox probe without applying the rejected global `ceil(canvas)+labelBounds/group-padding` formula.

## Evidence

- `cargo fmt --all`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_phase_names_refactor.md`
  remained the expected `-2.500px` residual.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_phase_names_refactor.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_phase_names_refactor.md`
  remained the expected 26-mismatch root report.
