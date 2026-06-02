# HPD-050 - Architecture Custom Init Final Elements

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Re-audited `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` with the enhanced
Architecture browser probe that now emits `finalElements`.

No renderer behavior changed in this slice.

## Evidence

Current focused local compare remains an expected root-only failure:

- `target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_current_debug.md`
- upstream max-width `325.105px`, local max-width `322.605px`
- upstream viewBox `325.105x380.479`, local viewBox `322.605x380.479`

Fresh browser probe:

- `target/compare/arch_batch6_init_fontsize_icon_size_wrap_probe_hpd050_final_elements.json`

The probe reports effective Architecture config:

- `iconSize=40`
- `fontSize=18`
- `padding=30`

It reports final upstream group bboxes:

- `left` `node.boundingBox()` `x1=-122.07139040868114`, `w=162`, `h=124`
- `right` `node.boundingBox()` `x1=-93.57139040868114`, `w=236.60543158585926`,
  `h=160.9244061401312`

The pinned upstream SVG group rects match those final bboxes after Mermaid's SVG group rect
translation:

- `left` rect `x=-102.07139040868114`, `w=162`, `h=124`
- `right` rect `x=-73.57139040868114`, `w=236.60543158585926`, `h=160.9244061401312`

The current local SVG emits:

- `left` rect `x=-77.60740329292963`, `w=159`, `h=124`
- `right` rect `x=-50.60740329292963`, `w=235.60543158585926`, `h=160.9244061401312`

So the focused root delta is not a height/root-finalize issue. The group widths are short by `3px`
for `left` and `1px` for `right`, while the overall root width is short by `2.5px` after local
component recentering.

The same finalElements block reports upstream service bboxes:

- `api` final `node.boundingBox().w=101`, `labelBounds.w=99`
- `db` final `node.boundingBox().w=84`, `labelBounds.w=82`
- `disk` final `node.boundingBox().w=42`, `labelBounds.w=39`

Local debug for the same fixture had already shown the tempting global formula class: widening
custom-init child service contribution can make this row exact, but the earlier exploratory
production formula expanded full Architecture root mismatches from `26` to `47`, so it was rejected
and reverted.

## Classification

This row remains a phase-specific Cytoscape service/group child bbox residual for custom
Architecture init settings. It is source-input-matched enough to keep auditing, but not safe enough
to fix with:

- a global group padding change,
- a single service label width scale,
- root viewport pins,
- fixture-specific text constants.

A valid fix needs a reusable model for the split between leaf service `node.boundingBox()`, child
contribution inside `updateCompoundBounds()`, final group `node.boundingBox()`, and local SVG root
bounds. Until that model holds across the broader Architecture queue, keep this row classified
rather than forcing it exact.
