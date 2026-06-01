# 2026-06-01 - M15C-070 Flowchart Long-Name C1 Root Slice

## Context

After the SVG-markdown shape-layout slice, strict Flowchart `parity-root` still reported 205
root-only mismatches. The largest remaining bucket was the long-name Flowchart fixtures:

- `upstream_cypress_flowchart_spec_12_should_render_a_flowchart_with_long_names_and_class_definitio_012`
- `upstream_cypress_flowchart_handdrawn_spec_fhd12_should_render_a_flowchart_with_long_names_and_class_defini_012`

The courier fixture had a `+96.050px` unpinned root max-width delta. The handdrawn/default-font
fixture had a `+98.490px` unpinned delta, and its stale root pin forced the default compare to
`+120.000px`.

## Diagnosis

Both fixtures use mojibake labels with preserved C1 control codepoints. The shared text measurer was
still treating those C1 controls as near-full-em fallback glyphs. Mermaid 11.15 Chromium output
measures them much closer to a narrow fallback for Flowchart HTML labels.

Changing the shared C1 fallback to approximately `0.6em` collapsed the large root drift:

- courier long-name fixture: `+96.050px` -> `+0.270px` unpinned
- handdrawn/default long-name fixture: `+98.490px` -> `+2.710px` unpinned

Exact text lookup overrides would make the two fixtures pass without root pins, but that grows the
text override footprint beyond the no-growth budget. The final slice therefore keeps the shared C1
fix and updates the two scoped 11.15 root pins for the remaining browser-font/root serialization
delta.

## Changes

- `crates/merman-render/src/text/font_metrics.rs`
  - Changed C1 fallback from near-full-em to `0.5987`.
- `crates/merman-render/src/text/tests.rs`
  - Updated `flowchart_html_c1_controls_measure_like_chromium_replacement_glyphs` to lock the new
    C1 fallback behavior.
- `crates/merman-render/src/generated/flowchart_root_overrides_11_12_2.rs`
  - Updated the handdrawn long-name root pin to the Mermaid 11.15 root.
  - Added a courier long-name root pin for the remaining subpixel root delta.

## Validation

- `cargo nextest run -p merman-render flowchart_html_c1_controls_measure_like_chromium_replacement_glyphs`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_spec_12_should_render_a_flowchart_with_long_names_and_class_definitio_012 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_handdrawn_spec_fhd12_should_render_a_flowchart_with_long_names_and_class_defini_012 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; text lookup overrides stayed at `490`, root viewport overrides are `282`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: still failed with 203 Flowchart root-only mismatches; both long-name rows report `+0.000px`.
- `cargo nextest run -p merman-render flowchart`: passed, 88 tests.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.

## Next

Continue M15C-070 with the new top Flowchart root buckets: shape-alias geometry, hexagon root
geometry, markdown subgraph root sizing, and small root-rounding clusters.
