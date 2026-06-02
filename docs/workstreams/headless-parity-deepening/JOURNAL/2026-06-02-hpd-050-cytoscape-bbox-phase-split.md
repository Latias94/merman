# HPD-050 Cytoscape BBox Phase Split Finding

Date: 2026-06-02

## Finding

The Architecture diagnostic probe can now expose Cytoscape's pre-layout label/body/group bbox
metrics. For `stress_architecture_batch6_init_fontsize_icon_size_wrap_093`, the shipped
Mermaid/Cytoscape path reports:

- `api` service: `labelWidth=95`, `labelHeight=18`, `labelBounds=99x22`,
  `bodyBounds=42x42`, and `node.boundingBox()=101x62`
- `db` service: `labelWidth=78`, `labelBounds=82x22`, and `node.boundingBox()=84x62`
- `disk` service: `labelWidth=35`, `labelBounds=39x22`, and `node.boundingBox()=42x62`
- `left` group: `autoWidth=99`, `autoHeight=61`, `outerWidth=160`, `outerHeight=122`,
  and `node.boundingBox()=162x124`

This confirms a useful source-backed distinction:

- a leaf service's default `node.boundingBox()` includes the final anti-alias expansion
- compound sizing uses the cached `bodyBounds` / `labelBounds` combination, not the same final
  default bbox
- group `node.boundingBox()` then adds compound padding, a border contribution, and a final
  anti-alias expansion

## Rejected Production Patch

An exploratory production patch translated the simple Cytoscape formula into
`architecture_metrics.rs`:

- `canvas_width = ceil(vendored_measure * existing_scale)`
- compound service width from `max(icon + 2, canvas_width + 4)`
- group bbox padding extra from `+2.5` to `+1.5`
- compound label bottom from `fontSize + 1` to `fontSize + 2`

Focused evidence looked attractive:

- `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` became root-exact:
  `325.105x380.479` upstream and local
- `stress_architecture_batch4_init_small_icons_061` stayed root-exact:
  `187.859x191.571` upstream and local

But the full Architecture root report regressed from 26 mismatches to 47 mismatches. Notable
examples:

- `stress_architecture_batch5_long_titles_and_punct_076` worsened from `+5.000px` to `+7.500px`
- `stress_architecture_html_titles_and_escapes_041` worsened from `+5.000px` to `+7.500px`
- many nested/group-heavy rows reopened or shifted by `0.5-7px`

The production patch was reverted. The only retained code change from this slice is the diagnostic
probe's extra metrics output.

## Rule

Do not apply a single global `ceil(canvas)+labelBounds/group-padding` formula to Architecture root
bounds yet. The next real implementation, if pursued, should introduce a phase-specific bbox model
that can separately represent:

- leaf default `node.boundingBox()`
- child contribution to `updateCompoundBounds()`
- final group `node.boundingBox()` used by `svgDraw.ts`
- `manatee` relocation bbox approximations

Until that split exists, the current broad heuristic is a better production tradeoff than a
locally exact custom-init rule.

## Evidence

- `target/compare/arch_batch6_init_fontsize_icon_size_wrap_probe_hpd050_metrics.json`
- `target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_cytoscape_bbox_seam_y.md`
- `target/compare/architecture_batch4_small_icons_hpd050_cytoscape_bbox_seam_y.md`
- `target/compare/architecture_report_parity_root_after_hpd050_cytoscape_bbox_seam.md`
- `target/compare/architecture_report_parity_root_after_hpd050_probe_metrics_only.md`
