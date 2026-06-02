# HPD-050 - Architecture Edge Label Bounds

Date: 2026-06-02

## Context

The second HPD-050 slice re-audited `stress_architecture_batch4_init_small_icons_061`. Earlier
evidence correctly showed the service labels were icon-floor dominated, but the root residual was
not caused by service/group sizing alone.

Mermaid source shows Architecture edge labels are emitted through `createText(...)`, then Y-axis
labels are rotated with `rotate(-90)`, and the final root viewport is derived from
`svg.getBBox()` in `setupGraphViewbox(...)`.

## Change

- Added `architecture_create_text_bbox_y_range_px(...)` to expose the local `createText` bbox
  y-range used by root-bounds estimation.
- Changed Architecture edge-label plans to carry transformed `Bounds` instead of centered
  `aabb_w/aabb_h` pairs.
- Corrected compound label bottom from `fontSize * 17 / 16` to the source-backed `fontSize + 1px`
  rule, preserving the default `16px -> 17px` behavior while fixing custom Architecture font sizes.
- Added a small-icons regression test that keeps service/group sizing icon-floor dominated while
  asserting that the vertical edge label contributes to root width.

## Evidence

- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_vertical_edge_label_bounds_use_create_text_y_offsets --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_small_icons_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

## Outcome

`stress_architecture_batch4_init_small_icons_061`,
`stress_architecture_batch4_init_fontsize_wrap_063`, and
`stress_architecture_edge_label_corner_cases_012` are root-green after this slice. Full Architecture
structural parity remains green. Architecture parity-root still fails with `26` mismatches; the top
remaining residuals are still `junction_fork_join_026`, `batch5_long_titles_and_punct_076`, and
`html_titles_and_escapes_041`.
