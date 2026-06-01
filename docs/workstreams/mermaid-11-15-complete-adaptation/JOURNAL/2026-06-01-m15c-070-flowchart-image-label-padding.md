# M15C-070 Flowchart Image Label Padding And Root Pins

Date: 2026-06-01
Task: M15C-070

## Summary

Closed the leading Flowchart strict-root `upstream_docs_flowchart_parameters_136` residual by
aligning image-shape label sizing with Mermaid 11.15. `imageSquare.ts` gets the label bbox from
`labelHelper(...)`, while the Flowchart stylesheet gives `.image-shape p` and `.icon-shape p`
`padding: 2px`. Local image-square layout and rendering used the unpadded label bbox, so the
single image fixture was exactly 4px too narrow and its label/image placement was 2px off.

Also refreshed three stale existing root pins after proving they were not renderer gaps:

- `stress_flowchart_shape_mix_009`: passes strict-root with root overrides disabled; the existing
  pin still forced the old `369.66796875x698.21875` viewport. The pin now matches the Mermaid
  11.15 upstream root `366.359375x703.21875`.
- `upstream_html_demos_flowchart_flowchart_010` and
  `upstream_html_demos_flowchart_flowchart_049`: root overrides were still pinned to
  `2004.41015625x1046`; Mermaid 11.15 upstream is `2007.41015625x1046`. With overrides disabled
  the renderer is already within the small browser/root residual (`2007.390` local vs
  `2007.410` upstream), so the existing pins were refreshed to the pinned 11.15 roots.

## Source References

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/imageSquare.ts`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/styles.ts`

## Validation

- `cargo nextest run -p merman-render flowchart_image_shape_label_bbox_includes_mermaid_padding`
  passed.
- `cargo nextest run -p merman-render flowchart_image_shape_label_bbox_includes_mermaid_padding flowchart_v2_fontawesome_edge_label_width_uses_nominal_icon_boundary`
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_docs_flowchart_parameters_136 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter stress_flowchart_shape_mix_009 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter stress_flowchart_shape_mix_009 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`
  passed after refreshing the existing pin.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_flowchart_010 --filter upstream_html_demos_flowchart_flowchart_049 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`
  passed after refreshing the existing pins.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`
  still fails as expected with 61 Flowchart strict root-only mismatches, down from 65.
- `cargo run -p xtask -- report-overrides --check-no-growth` passed; root viewport overrides
  remain at 282 total entries and text lookup overrides remain at 495.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
  passed.
- `cargo fmt --check` passed.
- `git diff --check` passed.

## Follow-Up

The new leading Flowchart strict-root buckets are markdown/html=false shape rows and small
subgraph/markdown/root-rounding residuals. Continue by checking whether the markdown rows share an
SVG-label root sizing rule before adding any more root pins.
