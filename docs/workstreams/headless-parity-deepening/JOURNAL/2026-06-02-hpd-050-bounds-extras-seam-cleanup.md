# HPD-050 Bounds Extras Seam Cleanup

Date: 2026-06-02

## Why

The remaining Architecture `+5px` rows
`stress_architecture_batch5_long_titles_and_punct_076` and
`stress_architecture_html_titles_and_escapes_041` looked tempting to close by changing the shared
compound padding helper. Fresh inspection showed that would be the wrong abstraction.

Saved Mermaid browser probes show upstream service positions match the probe, while local service
positions are only about `0.5px` off in X. The root deltas are controlled by final group rectangle
widths:

- `batch5_long_titles`: upstream `462.925633px`, local `467.925633px`
- `html_titles`: upstream `399.925633px`, local `404.925633px`

The old helper name `architecture_compound_bbox_padding_px(...)` hid two different Cytoscape
phases under one name: final SVG group rect approximation and layout-engine element/relocation
bbox approximation.

## Change

- Removed the renderer-side `initial_center` / pre-layout group bbox model from
  `architecture.rs`. It was not consumed by layout; `manatee` owns relocation-centering from the
  indexed graph adapter.
- Renamed the actual renderer-side helper to `architecture_svg_group_bbox_padding_px(...)`.
- Kept behavior unchanged for final group rectangles.
- Kept the two `+5px` rows open as group/service Cytoscape bbox measurement residuals.

## Evidence

- `cargo fmt --all`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_svg_group_bbox_padding_adds_headless_cytoscape_extra --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

## Outcome

Architecture structural parity remains green. Architecture `parity-root` still has `26` mismatches,
with the same top residual family. This is an intentional no-count refactor: it reduces misleading
measurement code and keeps future fixes honest.
