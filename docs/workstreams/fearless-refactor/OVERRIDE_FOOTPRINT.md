# Override Footprint

This file records the generated and manual parity override footprint so growth is visible during
fearless refactor work. Overrides are useful when upstream behavior depends on browser/font
measurement or a temporary raw SVG/path compatibility bridge, but new model fixes should be
preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-09

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

Generated override modules scanned: `37`.

Manual raw SVG/path bridge files scanned: `1`.

### Category Metadata Snapshot

`xtask report-overrides` now prints category-level owner, source, allowed-use, and expected-removal
metadata before each count table. This keeps removal criteria visible in CI logs and drift reviews,
not only in policy prose.

The same category totals are encoded as no-growth budgets in
`cargo run -p xtask -- report-overrides --check-no-growth`, which is part of the strict release
gate. Override growth should therefore be an explicit reviewed decision, not a default model-bug
escape hatch.

The current snapshot reflects a 34-entry reduction in root viewport overrides after topology-driven
viewport calibration replaced several fixture-specific root pins, the `journey` root viewport
overrides were removed entirely, and profile-based `kanban` root height calibration replaced the
remaining fixture-specific Kanban root pins.
It also reflects corrected text-lookup accounting: generated `*_OVERRIDES_*` binary-search tables
in `block`, `er`, `gantt`, and `mindmap` are now counted as text metric lookup entries instead of
hand-curated helper functions.
The hand-curated helper total also reflects pruning two redundant public Sankey padding component
helpers; the remaining public Sankey helper is the actual `showValues`-aware padding lookup used by
layout tests and render code.

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

Total entries reported by `xtask`: `1540`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 101 |
| `block_root_overrides_11_12_2.rs` | 119 |
| `c4_root_overrides_11_12_2.rs` | 51 |
| `class_root_overrides_11_12_2.rs` | 196 |
| `er_root_overrides_11_12_2.rs` | 35 |
| `flowchart_root_overrides_11_12_2.rs` | 266 |
| `gitgraph_root_overrides_11_12_2.rs` | 232 |
| `mindmap_root_overrides_11_12_2.rs` | 80 |
| `pie_root_overrides_11_12_2.rs` | 35 |
| `requirement_root_overrides_11_12_2.rs` | 46 |
| `sankey_root_overrides_11_12_2.rs` | 7 |
| `sequence_root_overrides_11_12_2.rs` | 232 |
| `state_root_overrides_11_12_2.rs` | 122 |
| `timeline_root_overrides_11_12_2.rs` | 18 |

Largest root-viewport buckets:

- `flowchart`: 266
- `gitgraph`: 232
- `sequence`: 232
- `class`: 196

### Text Metric Lookup Overrides

Total lookup entries reported by `xtask`: `1140`.

| file | lookup entries |
| --- | ---: |
| `block_text_overrides_11_12_2.rs` | 125 |
| `c4_text_overrides_11_12_2.rs` | 3 |
| `class_text_overrides_11_12_2.rs` | 342 |
| `er_text_overrides_11_12_2.rs` | 114 |
| `flowchart_text_overrides_11_12_2.rs` | 48 |
| `gantt_text_overrides_11_12_2.rs` | 44 |
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

Total helper functions reported by `xtask`: `68`.

| file | helper functions |
| --- | ---: |
| `architecture_text_overrides_11_12_2.rs` | 7 |
| `gitgraph_text_overrides_11_12_2.rs` | 6 |
| `journey_text_overrides_11_12_2.rs` | 13 |
| `kanban_text_overrides_11_12_2.rs` | 5 |
| `pie_text_overrides_11_12_2.rs` | 9 |
| `radar_text_overrides_11_12_2.rs` | 3 |
| `sankey_text_overrides_11_12_2.rs` | 5 |
| `sequence_text_overrides_11_12_2.rs` | 9 |
| `treemap_text_overrides_11_12_2.rs` | 8 |
| `xychart_text_overrides_11_12_2.rs` | 3 |

### Manual Raw SVG/Path Bridges

Total bridge functions reported by `xtask`: `1`.

| file | bridge functions |
| --- | ---: |
| `svg/parity/flowchart/edge_geom/degenerate_path.rs` | 1 |

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

- Manual bridge scanning is intentionally naming-based today: hand-authored bridge functions must
  use `maybe_override_*` under `crates/merman-render/src/svg/parity/` to be visible.
- Generated override metadata is category-level. Per-entry fixture/probe provenance still lives in
  generator inputs, generated comments, tests, and upstream fixture names.

## Next Actions

- Keep temporary raw SVG/path bridge functions named `maybe_override_*` and documented with
  owner/removal notes.
- Review the largest root-viewport buckets before adding new entries, especially `flowchart`,
  `gitgraph`, `sequence`, and `class`.
- Tighten per-entry fixture/probe provenance when regenerating large override tables.
