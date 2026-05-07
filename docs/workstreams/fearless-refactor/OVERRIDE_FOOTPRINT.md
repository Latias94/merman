# Override Footprint

This file records the generated parity override footprint so growth is visible during fearless
refactor work. Overrides are useful when upstream behavior depends on browser/font measurement, but
new model fixes should be preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-07

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

Generated override modules scanned: `39`.

### Root Viewport Overrides

Total entries reported by `xtask`: `1574`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 120 |
| `block_root_overrides_11_12_2.rs` | 119 |
| `c4_root_overrides_11_12_2.rs` | 51 |
| `class_root_overrides_11_12_2.rs` | 196 |
| `er_root_overrides_11_12_2.rs` | 35 |
| `flowchart_root_overrides_11_12_2.rs` | 266 |
| `gitgraph_root_overrides_11_12_2.rs` | 232 |
| `journey_root_overrides_11_12_2.rs` | 4 |
| `kanban_root_overrides_11_12_2.rs` | 11 |
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

Total lookup arms reported by `xtask`: `567`.

| file | lookup arms |
| --- | ---: |
| `c4_text_overrides_11_12_2.rs` | 3 |
| `class_text_overrides_11_12_2.rs` | 342 |
| `er_text_overrides_11_12_2.rs` | 1 |
| `flowchart_text_overrides_11_12_2.rs` | 48 |
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

Total helper functions reported by `xtask`: `90`.

| file | helper functions |
| --- | ---: |
| `architecture_text_overrides_11_12_2.rs` | 8 |
| `block_text_overrides_11_12_2.rs` | 2 |
| `gantt_text_overrides_11_12_2.rs` | 2 |
| `gitgraph_text_overrides_11_12_2.rs` | 6 |
| `journey_text_overrides_11_12_2.rs` | 15 |
| `kanban_text_overrides_11_12_2.rs` | 7 |
| `mindmap_text_overrides_11_12_2.rs` | 1 |
| `pie_text_overrides_11_12_2.rs` | 13 |
| `radar_text_overrides_11_12_2.rs` | 4 |
| `sankey_text_overrides_11_12_2.rs` | 7 |
| `sequence_text_overrides_11_12_2.rs` | 10 |
| `treemap_text_overrides_11_12_2.rs` | 11 |
| `xychart_text_overrides_11_12_2.rs` | 4 |

### Counting Notes

Counts are inventory units and should not be compared directly across categories:

- Root viewport entries count match arms returning `Some((viewBox, max_width))`.
- Text metric lookup arms count `=> Some(...)` parity branches.
- Font/SVG metric table rows count tuple rows in generated lookup arrays.
- Helper overrides count public helper functions in generated small constant modules.

## Categories

- Root viewport overrides: fixture-derived `viewBox` / `max-width` pins for browser float and
  emitted-bounds drift.
- Generated text metrics: per-diagram width/height/bbox constants or lookup tables derived from
  upstream browser measurements.
- Raw SVG/path precision bridges: temporary geometry/path literal bridges, currently tracked in
  older parity workstream notes rather than by `xtask report-overrides`.
- Hand-curated constants: small stable constants such as sequence frame/text spacing helpers.

## Known Gaps

- `xtask report-overrides` now scans generated override modules, but does not yet inventory
  temporary raw SVG/path precision bridges outside `crates/merman-render/src/generated/`.
- The report does not yet separate temporary bridge overrides from long-lived browser/font
  compatibility data.
- Removal criteria are not encoded in generated metadata yet.

## Next Actions

- Add owner/removal notes for temporary raw SVG/path precision bridges.
- Review the largest root-viewport buckets before adding new entries, especially `flowchart`,
  `gitgraph`, `sequence`, and `class`.
- Add generated metadata when an override has an expected removal condition.
