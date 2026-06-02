# HPD-050 - Architecture Final BBox Probe

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Enhanced `tools/debug/arch_fcose_browser_probe_fixture_025.js` so it now emits
`finalElements` after the second FCoSE run.

The existing `final` service-position summary is preserved for compatibility. The new
`finalElements` block mirrors the existing `preLayout` dump shape and includes node/edge bboxes,
label bounds, body bounds, classes, data, and relevant metrics after the Mermaid-style segment
adjustment and second layout pass.

## Why

The Architecture `+5px` and `+3px` root residuals are dominated by final compound/group
`node.boundingBox()` behavior. Before this change, audits had to infer final group bboxes from the
stored upstream SVG group rect (`rect.x = node.boundingBox().x1 + iconSize / 2`). That inference is
easy to get wrong when service positions, child label bounds, group padding, and SVG root bbox all
interact.

The probe now exposes the final Cytoscape facts directly.

## Evidence

`stress_architecture_unicode_and_xml_escapes_019`:

- `target/compare/arch_unicode_xml_probe_hpd050_final_elements.json` reports final group `i`
  `node.boundingBox()` as `x1=-209.91096759368116`, `w=389.8219351873623`.
- Adding `iconSize / 2 = 40` gives SVG group rect `x=-169.91096759368116`, matching the pinned
  upstream SVG group rect.
- The same finalElements block reports `Metrics Exporter` `labelBounds.w=121` and
  `node.boundingBox().w=123`, while local headless child bounds currently estimate that controlling
  label wider.

`stress_architecture_batch5_long_titles_and_punct_076`:

- `target/compare/arch_batch5_long_titles_probe_hpd050_final_elements.json` reports final group
  `pipeline` `node.boundingBox()` as `x1=-273.4628163140578`, `w=462.92563262811564`.
- Adding `iconSize / 2 = 40` gives SVG group rect `x=-233.4628163140578`, matching the pinned
  upstream SVG group rect.
- The controlling `Artifacts Storage retention 30d` service reports `labelBounds.w=221` and
  `node.boundingBox().w=223`.

## Next Use

Use `finalElements` for Architecture residual classification before changing renderer or manatee
math. It should help separate:

- service placement drift,
- child service label-bound drift,
- final group `node.boundingBox()` drift,
- root `setupGraphViewbox(...)` drift.

Do not replace this with root-width pins or fixture-specific text constants.
