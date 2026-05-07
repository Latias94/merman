# Override Footprint

This file records the generated parity override footprint so growth is visible during fearless
refactor work. Overrides are useful when upstream behavior depends on browser/font measurement, but
new model fixes should be preferred when the mismatch comes from our own data or geometry.

## Snapshot: 2026-05-07

Command:

`cargo run -p xtask -- report-overrides`

Mermaid baseline: `@11.12.3`

### Root Viewport Overrides

Total entries reported by `xtask`: `1523`.

| file | entries |
| --- | ---: |
| `architecture_root_overrides_11_12_2.rs` | 120 |
| `block_root_overrides_11_12_2.rs` | 119 |
| `flowchart_root_overrides_11_12_2.rs` | 266 |
| `class_root_overrides_11_12_2.rs` | 196 |
| `mindmap_root_overrides_11_12_2.rs` | 80 |
| `gitgraph_root_overrides_11_12_2.rs` | 232 |
| `journey_root_overrides_11_12_2.rs` | 4 |
| `er_root_overrides_11_12_2.rs` | 35 |
| `kanban_root_overrides_11_12_2.rs` | 11 |
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

### Text / BBox Overrides Reported By `xtask`

| file | entries |
| --- | ---: |
| `flowchart_text_overrides_11_12_2.rs` | 48 |
| `state_text_overrides_11_12_2.rs` | 46 |

## Categories

- Root viewport overrides: fixture-derived `viewBox` / `max-width` pins for browser float and
  emitted-bounds drift.
- Generated text metrics: per-diagram width/height/bbox constants or lookup tables derived from
  upstream browser measurements.
- Raw SVG/path precision bridges: temporary geometry/path literal bridges, currently tracked in
  older parity workstream notes rather than by `xtask report-overrides`.
- Hand-curated constants: small stable constants such as sequence frame/text spacing helpers.

## Known Gaps

- `xtask report-overrides` is intentionally lightweight and does not yet count every generated text
  constant table under `crates/merman-render/src/generated/`.
- The report does not yet separate temporary bridge overrides from long-lived browser/font
  compatibility data.
- Removal criteria are not encoded in generated metadata yet.

## Next Actions

- Expand `xtask report-overrides` to scan all generated override tables by category.
- Add owner/removal notes for temporary raw SVG/path precision bridges.
- Review the largest root-viewport buckets before adding new entries, especially `flowchart`,
  `gitgraph`, `sequence`, and `class`.
