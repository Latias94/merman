# Override Footprint

This file records the generated and manual parity override footprint so growth is visible during
fearless refactor work. Overrides are useful when upstream behavior depends on browser/font
measurement or a temporary raw SVG/path compatibility bridge, but new model fixes should be
preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-10

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

Generated override modules scanned: `22`.

Manual raw SVG/path bridge files scanned: `0`.

### Category Metadata Snapshot

`xtask report-overrides` now prints category-level owner, source, allowed-use, and expected-removal
metadata for every category, including zero-count categories. This keeps removal criteria and
successful eliminations visible in CI logs and drift reviews, not only in policy prose.

The same category totals are encoded as no-growth budgets in
`cargo run -p xtask -- report-overrides --check-no-growth`, which is part of the strict release
gate. Override growth should therefore be an explicit reviewed decision, not a default model-bug
escape hatch. Manual raw SVG/path bridges now have an exact zero budget, so any bridge
reintroduction fails the strict gate unless the budget is deliberately changed.

The current snapshot reflects a 795-entry net reduction in root viewport overrides after topology-driven
viewport calibration replaced several fixture-specific root pins, the `journey` root viewport
overrides were removed entirely, and profile-based `kanban` root height calibration replaced the
remaining fixture-specific Kanban root pins, followed by four obsolete Sankey pins that now match
deterministic emitted bounds and nine obsolete Timeline pins that now match deterministic root
output, then twelve obsolete Pie pins and twelve obsolete ER pins that also match deterministic
root output, thirty-five obsolete Requirement pins, and sixteen obsolete C4 pins now covered by
deterministic root output. It also reflects deletion of the now-empty Block root override module
after all 119 entries proved obsolete, followed by sixty-eight obsolete State pins now covered by
deterministic root output. It then collapses the Class root table from 196 entries to 31 by
removing 166 obsolete pins and adding one missing docs root pin, making Class `parity-root` green
with a 165-entry net reduction, followed by six obsolete Gitgraph pins now covered by
deterministic root output and thirty-two obsolete Sequence pins now covered by deterministic root
output. Flowchart then dropped 131 obsolete pins and later cleared the
`upstream_docs_math_flowcharts_001` `parity-root` gap by normalizing the browser-sensitive math SVG
baseline and measuring sanitized KaTeX MathML through the Node probe. Pie then replaced its 23
remaining root pins with a typed empty-pie root viewport rule plus shared 1/64px-quantized legend
SVG bbox measurement, deleting the Pie root override module. Mindmap then refreshed typed root
profile calibration, added two small model-derived root profiles, and pruned 28 obsolete root pins
while keeping `parity-root` green. Class then moved its remaining 31 root viewport pins into typed
profile calibration and namespace render-mode rules, deleting the Class root override module while
keeping `parity-root` green. Architecture then added default root viewport calibration for the
nested-groups and reasonable-height profiles and pruned 70 obsolete fixture-scoped pins, leaving 31
Architecture root pins that still guard measured `parity-root` drift. It also reflects the final
manual raw SVG/path bridge removal, so manual bridge scanning now reports zero bridge files. It
also reflects corrected text-lookup accounting: generated `*_OVERRIDES_*` binary-search tables in
`block`, `er`, and `gantt` are now counted as text metric lookup entries instead of hand-curated
helper functions. Text lookup ownership has also tightened: ER and Block generated HTML width
tables are no longer consulted by the generic vendored text measurer, and their call sites now
live in their owning diagram modules. The stale Mindmap HTML width table was deleted after layout
snapshots proved the stable Mindmap path does not use it, reducing text lookup debt by 291 entries
while preventing shared text measurement from leaking fixture-specific widths across diagrams. C4
then moved its three per-line SVG bbox height rules into the C4 owner module and deleted the
generated `c4_text_overrides_11_12_2.rs` module, reducing text lookup debt by another 3 entries.
Gantt then dropped the generic `A`, `B`, `C`, `Build`, `Design`, `Noon`, `t1`, `task1`, `test1`,
and `test2` task-width overrides after the font-metric fallback proved stable, reducing text lookup
debt by ten more entries. A follow-up pass dropped `test3` through `test7`, reducing debt by five
more entries. A final pass dropped `task2` through `task4`, reducing debt by three more entries.
The isolated `y68` and `y69` labels then dropped, reducing debt by two more entries. C4 then moved
its 17 type-line `textLength` pins into the C4 owner
module and deleted the generated `c4_type_textlength_11_12_2.rs` module, so C4 type-line
`textLength` now lives in owner code instead of the override inventory.
The hand-curated helper total also reflects pruning two redundant public Sankey padding component
helpers before the remaining Sankey node geometry moved back to the `sankey` owner module.
Since then, Pie inlined its fixed margin, center, radius, label font size, title y, and legend
text y literals, Sequence now derives its note padding total from the existing note gap, Journey
inlined its single-use legend placement and mouth offset values, Radar inlined its remaining
legend box size and label x-offset literals, XYChart inlined its bar data-label scale and inset
literals, deleting the empty generated override module, Sequence inlined its self-only frame min pad
literals in block geometry, Sankey inlined its SVG-only label font/gap/dy literals, Treemap
inlined its section header label/value sizing literals, Architecture deleted a dead icon text bbox
helper, and Radar inlined its final legend row spacing value and deleted the now-empty generated
module. Pie also moved its remaining legend rectangle/spacing values into `pie` owner constants and
deleted the now-empty generated module. Sankey then moved its node width/padding values into
`sankey` owner constants and deleted the now-empty generated module, bringing the hand-curated
helper total to 32. Journey moved its fixed viewBox/title/legend/face geometry into `journey`
owner constants and deleted the now-empty generated module, bringing the hand-curated helper total
to 26. Kanban moved its section padding, label foreignObject height, and item row heights into
`kanban` owner constants and deleted the now-empty generated module, bringing the hand-curated
helper total to 21. Treemap moved its section spacing geometry into `treemap` owner constants and
kept the remaining `Item A1` leaf-fit browser tolerance beside the SVG parity loop, deleting the
now-empty generated module and bringing the hand-curated helper total to 18. Sequence moved its
note wrap slack, text line-height math, and frame padding geometry into `sequence` owner constants
and functions, deleting the now-empty generated module and bringing the hand-curated helper total
to 12. Architecture moved its text bbox formulas, canvas-label width scale, service label
extension, and default wrap width into `architecture` owner constants/functions, deleting the
now-empty generated module and bringing the hand-curated helper total to 6. Gitgraph then moved
its branch-label correction control flow into the `gitgraph` owner module and reclassified its
bbox correction data as text metric lookup entries, bringing the hand-curated helper total to 0.

| category | owner | expected removal |
| --- | --- | --- |
| Root viewport overrides | render parity workstream | Delete entries once typed layout/emitted bounds can derive the same root viewport or a baseline upgrade removes the pinned behavior. |
| Text metric lookup overrides | render parity workstream | Delete entries once vendored/shared text measurement returns the upstream dimensions without fixture-specific lookup arms. |
| SVG text metric tables | render parity workstream | Replace with shared font metrics or browser-probe imports, then delete stale rows. |
| Font metric tables | shared text measurement owner | Regenerate or trim when better vendored font/probe data covers the drift; remove only if a real measurement backend becomes the default. |
| Hand-curated helper overrides | diagram renderer owner | Replace with repeatable generated data or typed model/layout computations as soon as a reliable source exists. |
| Manual raw SVG/path bridges | diagram-specific `svg/parity` module owner | Delete once typed layout/path emission reproduces the upstream literal behavior; keep local owner/removal notes beside each bridge. |

### Root Viewport Overrides

Total entries reported by `xtask`: `779`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 31 |
| `c4_root_overrides_11_12_2.rs` | 35 |
| `er_root_overrides_11_12_2.rs` | 23 |
| `flowchart_root_overrides_11_12_2.rs` | 135 |
| `gitgraph_root_overrides_11_12_2.rs` | 226 |
| `mindmap_root_overrides_11_12_2.rs` | 52 |
| `requirement_root_overrides_11_12_2.rs` | 11 |
| `sankey_root_overrides_11_12_2.rs` | 3 |
| `sequence_root_overrides_11_12_2.rs` | 200 |
| `state_root_overrides_11_12_2.rs` | 54 |
| `timeline_root_overrides_11_12_2.rs` | 9 |

Sankey note: the remaining 3 root viewport entries were rechecked by disabling the Sankey root
lookup and running `compare-sankey-svgs --check-dom --dom-mode parity-root --dom-decimals 3`.
They still cover root height drift in
`upstream_docs_sankey_example_002`, `upstream_examples_sankey_energy_flow_001`, and
`upstream_html_demos_sankey_energy_flow_002`, so they are not redundant yet.

Small-bucket audit note: disabling the remaining Timeline, Requirement, and ER root lookups showed
their surviving entries still guard real `parity-root` drift. Pie was the exception; its root bucket
is now deleted after empty-pie viewport and legend bbox behavior moved into typed renderer logic.

Mindmap note: after the typed profile refresh, disabling the remaining Mindmap root lookup still
leaves 52 `parity-root` mismatches. Those entries stay in the budget until their geometry/text
profiles move into typed renderer logic.

Gitgraph and Flowchart audit note: a 2026-05-09 recheck confirmed that disabling the Gitgraph
direct root lookup still leaves all 226 Gitgraph root entries failing, and disabling the shared
root override helper still leaves all 135 Flowchart root entries failing. These buckets need
root-viewport derivation work before table pruning, not another blind deletion pass.

Largest root-viewport buckets:

- `gitgraph`: 226
- `sequence`: 200
- `flowchart`: 135
- `state`: 54
- `mindmap`: 52

### Text Metric Lookup Overrides

Total lookup entries reported by `xtask`: `860`.

| file | lookup entries |
| --- | ---: |
| `block_text_overrides_11_12_2.rs` | 125 |
| `class_text_overrides_11_12_2.rs` | 342 |
| `er_text_overrides_11_12_2.rs` | 114 |
| `flowchart_text_overrides_11_12_2.rs` | 48 |
| `gantt_text_overrides_11_12_2.rs` | 24 |
| `gitgraph_text_overrides_11_12_2.rs` | 34 |
| `requirement_text_overrides_11_12_2.rs` | 126 |
| `state_text_overrides_11_12_2.rs` | 46 |
| `timeline_text_overrides_11_12_2.rs` | 1 |

### SVG Text Metric Tables

| file | table rows |
| --- | ---: |
| `svg_overrides_sequence_11_12_2.rs` | 184 |

### Font Metric Tables

| file | table rows |
| --- | ---: |
| `font_metrics_flowchart_11_12_2.rs` | 3774 |

### Hand-Curated Helper Overrides

Total helper functions reported by `xtask`: `0`.

No hand-curated helper override modules remain.

### Manual Raw SVG/Path Bridges

Total bridge functions reported by `xtask`: `0`.

No manual raw SVG/path bridges remain.

### Counting Notes

Counts are inventory units and should not be compared directly across categories:

- Root viewport entries count match arms returning `Some((viewBox, max_width))`.
- Text metric lookup entries count `=> Some(...)` parity branches and rows in generated
  `*_OVERRIDES_*` binary-search tables.
- Font/SVG metric table rows count tuple rows in generated lookup arrays.
- Helper overrides count public helper functions in generated small constant modules.
- Manual raw SVG/path bridge counts are hand-authored `maybe_override_*` functions under
  `crates/merman-render/src/svg/parity/`.
- Category metadata in the report records owner, source, allowed use, and expected removal criteria
  for every generated/manual override category.

## Categories

- Root viewport overrides: fixture-derived `viewBox` / `max-width` pins for browser float and
  emitted-bounds drift.
- Generated text metrics: per-diagram width/height/bbox constants or lookup tables derived from
  upstream browser measurements.
- Raw SVG/path precision bridges: temporary hand-authored geometry/path literal bridges tracked by
  `xtask report-overrides` when named `maybe_override_*`.
- Hand-curated constants: small stable constants such as sequence frame/text spacing helpers.

## Known Gaps

- Manual bridge scanning is intentionally naming-based today: any future hand-authored bridge
  functions must use `maybe_override_*` under `crates/merman-render/src/svg/parity/` to be visible.
- Generated override metadata is category-level. Per-entry fixture/probe provenance still lives in
  generator inputs, generated comments, tests, and upstream fixture names.

## Next Actions

- Keep any future temporary raw SVG/path bridge functions named `maybe_override_*` and documented
  with owner/removal notes.
- Review the largest root-viewport buckets before adding new entries, especially `flowchart`,
  `gitgraph`, `sequence`, `state`, and `mindmap`.
- Do not spend another cleanup pass on the 3 remaining Sankey root pins until Sankey root height
  derivation changes; the May 2026 recheck proved they still guard real `parity-root` drift.
- Do not spend another blind table-pruning pass on Gitgraph or Flowchart root pins until their root
  viewport derivation changes; the May 2026 recheck proved both buckets still guard real
  `parity-root` drift.
- Tighten per-entry fixture/probe provenance when regenerating large override tables.
