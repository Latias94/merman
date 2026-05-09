# Override Footprint

This file records the generated and manual parity override footprint so growth is visible during
fearless refactor work. Overrides are useful when upstream behavior depends on browser/font
measurement or a temporary raw SVG/path compatibility bridge, but new model fixes should be
preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-09

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

Generated override modules scanned: `27`.

Manual raw SVG/path bridge files scanned: `0`.

### Category Metadata Snapshot

`xtask report-overrides` now prints category-level owner, source, allowed-use, and expected-removal
metadata for every category, including zero-count categories. This keeps removal criteria and
successful eliminations visible in CI logs and drift reviews, not only in policy prose.

The same category totals are encoded as no-growth budgets in
`cargo run -p xtask -- report-overrides --check-no-growth`, which is part of the strict release
gate. Override growth should therefore be an explicit reviewed decision, not a default model-bug
escape hatch.

The current snapshot reflects a 512-entry net reduction in root viewport overrides after topology-driven
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
output. It also reflects the final manual raw SVG/path bridge
removal, so manual bridge scanning now reports zero bridge files.
It also reflects corrected text-lookup accounting: generated `*_OVERRIDES_*` binary-search tables
in `block`, `er`, `gantt`, and `mindmap` are now counted as text metric lookup entries instead of
hand-curated helper functions.
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
| Typed textLength lookups | C4 renderer owner | Delete once C4 type-line measurement is computed from shared text measurement or Mermaid stops emitting the pinned `textLength`. |
| Hand-curated helper overrides | diagram renderer owner | Replace with repeatable generated data or typed model/layout computations as soon as a reliable source exists. |
| Manual raw SVG/path bridges | diagram-specific `svg/parity` module owner | Delete once typed layout/path emission reproduces the upstream literal behavior; keep local owner/removal notes beside each bridge. |

### Root Viewport Overrides

Total entries reported by `xtask`: `1062`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 101 |
| `c4_root_overrides_11_12_2.rs` | 35 |
| `class_root_overrides_11_12_2.rs` | 31 |
| `er_root_overrides_11_12_2.rs` | 23 |
| `flowchart_root_overrides_11_12_2.rs` | 266 |
| `gitgraph_root_overrides_11_12_2.rs` | 226 |
| `mindmap_root_overrides_11_12_2.rs` | 80 |
| `pie_root_overrides_11_12_2.rs` | 23 |
| `requirement_root_overrides_11_12_2.rs` | 11 |
| `sankey_root_overrides_11_12_2.rs` | 3 |
| `sequence_root_overrides_11_12_2.rs` | 200 |
| `state_root_overrides_11_12_2.rs` | 54 |
| `timeline_root_overrides_11_12_2.rs` | 9 |

Largest root-viewport buckets:

- `flowchart`: 266
- `gitgraph`: 226
- `sequence`: 200
- `architecture`: 101

### Text Metric Lookup Overrides

Total lookup entries reported by `xtask`: `1174`.

| file | lookup entries |
| --- | ---: |
| `block_text_overrides_11_12_2.rs` | 125 |
| `c4_text_overrides_11_12_2.rs` | 3 |
| `class_text_overrides_11_12_2.rs` | 342 |
| `er_text_overrides_11_12_2.rs` | 114 |
| `flowchart_text_overrides_11_12_2.rs` | 48 |
| `gantt_text_overrides_11_12_2.rs` | 44 |
| `gitgraph_text_overrides_11_12_2.rs` | 34 |
| `mindmap_text_overrides_11_12_2.rs` | 291 |
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

### Typed TextLength Lookups

| file | lookup arms |
| --- | ---: |
| `c4_type_textlength_11_12_2.rs` | 17 |

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
  `gitgraph`, `sequence`, and `class`.
- Tighten per-entry fixture/probe provenance when regenerating large override tables.
