# HPD-050 - Architecture Group Port Final Elements

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Re-audited `stress_architecture_group_port_edges_017` with the enhanced Architecture browser probe
that now emits `finalElements`.

No renderer behavior changed in this slice.

## Evidence

Current focused local compare remains an expected root-only failure:

- `target/compare/architecture_group_port_edges_hpd050_current_debug.md`
- upstream max-width `707.769px`, local max-width `709.238px`
- upstream viewBox `707.769x542.448`, local viewBox `709.238x524.603`

Fresh browser probe:

- `target/compare/arch_group_port_edges_probe_hpd050_final_elements.json`

The probe reports final upstream service positions:

- `in1=(-6.610884535349662,117.224040623886)`
- `in2=(193.38461135170346,117.224040623886)`
- `out1=(-6.610884535349662,-121.72404062388597)`
- `ext=(-270.3846113517034,-121.72404062388597)`

The current local SVG emits:

- `in1=(-5.906611585551893,108.30146940356512)`
- `in2=(194.11875930058488,108.30146940356512)`
- `out1=(-5.906611585551893,-112.80146940356514)`
- `ext=(-271.1187593005849,-112.80146940356514)`

So local X spread is wider by about `1.468px`, while local Y spacing is compressed by about
`17.845px`. This exactly explains the root-width and root-height deltas.

The same probe confirms the source-side group/service bboxes are ordinary:

- `outer` final `node.boundingBox()` is `447.995x462.448`.
- `inner` final `node.boundingBox()` is `364.995x182`.
- all service nodes are icon-floor dominated `82x100` bboxes.

## Classification

This row remains source-input-matched manatee-vs-Cytoscape FCoSE solution / compound-bound drift.
It is not evidence for changing:

- group-edge shift constants,
- Architecture SVG edge emission,
- service label measurement,
- final root viewBox logic.

Do not tune this row in isolation. A valid fix would need a reusable FCoSE/compound-bound rule that
also holds across the broader Architecture residual queue.
