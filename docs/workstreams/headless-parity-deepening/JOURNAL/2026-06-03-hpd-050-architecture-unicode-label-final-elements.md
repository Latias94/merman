# HPD-050 - Architecture Unicode Label Final Elements

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Re-audited `stress_architecture_unicode_and_xml_escapes_019` with the enhanced Architecture
browser probe, local group-rect debug output, and focused text measurement checks.

No renderer behavior changed in this slice.

## Evidence

Current focused local compare remains an expected root-only failure:

- `target/compare/architecture_unicode_xml_hpd050_current_debug.md`
- upstream max-width `469.822px`, local max-width `472.822px`
- upstream viewBox `469.822x463.593`, local viewBox `472.822x463.593`

Fresh browser probe:

- `target/compare/arch_unicode_xml_probe_hpd050_final_elements.json`

Browser final group bbox:

- group `i` `node.boundingBox().x1=-209.91096759368116`, `w=389.8219351873623`,
  `h=383.5932371253472`

The pinned upstream SVG group rect matches that final bbox after Mermaid's SVG group rect
translation:

- group `i` rect `x=-169.91096759368116`, `w=389.8219351873623`,
  `h=383.5932371253472`

Current local SVG emits:

- group `i` rect `x=-174.41096759368116`, `w=392.8219351873624`,
  `h=383.5932371253471`

Browser final service bboxes:

- `metrics` `node.boundingBox().w=123`, `labelBounds.w=121`
- `logs` `node.boundingBox().w=101`, `labelBounds.w=99`
- `store` `node.boundingBox().w=93`, `labelBounds.w=91`
- `alert` `node.boundingBox().w=97`, `labelBounds.w=95`

Current local service positions are all about `-1.5px` on X compared with the browser probe, while
Y values match. The final root width delta is therefore controlled by the group/service child bbox
estimate rather than by edge path emission or height/root finalization.

Focused vendored text measurements:

- `Metrics Exporter`: `118.0546875`
- `Log Collector`: `94.9453125`
- `Store Query`: `84.7734375`
- `Alert Service`: `91.9609375`

Local group debug reports:

- `metrics` child bounds width `125`
- `logs` child bounds width `100`
- `store` child bounds width `89`
- `alert` child bounds width `97`
- group `i` content `(-131.91096759368116,-83.7966185626736)-(175.91096759368122,214.79661856267353)`,
  `pad=42.5`
- group `i` final width `392.8219351873624`

The stored upstream and local SVGs emit the same decoded label words for this fixture. The row name
is historical; the current residual is not an XML entity or label-decode issue.

## Classification

This row remains a service label / group child bbox phase residual. It is not evidence for changing:

- Architecture parser escaping,
- text decode/entity handling,
- SVG group rect translation,
- root viewport finalization,
- edge path emission.

The mixed evidence also rejects a single global text-width formula: vendored text widths, browser
`labelBounds`, and local group-child bounds are not separated by one stable offset or scale. A valid
fix needs the same phase-specific Cytoscape service/group bbox model required by the neighboring
Architecture residuals.
