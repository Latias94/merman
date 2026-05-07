# Render Model Inventory

This inventory tracks the semantic transport boundary between `merman-core` and
`merman-render`. It is intentionally about render-pipeline ownership, not parser completeness.

## Modes

- `typed-first`: `Engine::parse_diagram_for_render_model_sync` returns a dedicated
  `RenderSemanticModel` variant, and layout/SVG dispatch consume that typed model directly.
- `json-fallback`: `Engine::parse_diagram_for_render_model_sync` returns
  `RenderSemanticModel::Json`; layout/SVG dispatch still consume the semantic JSON model.
- `compat-json`: legacy render-only JSON API surface. This is only acceptable as a temporary
  compatibility bridge when a typed model already exists.

## Current State

| Diagram ids | Mode | Render dispatch | Migration priority |
| --- | --- | --- | --- |
| `flowchart-v2`, `flowchart`, `flowchart-elk` | `typed-first` | typed layout + typed SVG | Keep as the pattern for future migrations. |
| `stateDiagram`, `state` | `typed-first` | typed layout + typed SVG | Obsolete JSON-for-render compatibility helpers removed. |
| `classDiagram`, `class` | `typed-first` | typed layout + typed SVG | Keep typed model stable while splitting renderer modules. |
| `mindmap` | `typed-first` | typed layout + typed SVG | Obsolete JSON-for-render compatibility helpers removed. |
| `architecture` | `typed-first` | typed layout + typed SVG | Keep typed model stable while splitting renderer modules. |
| `sequence` | `typed-first` | typed layout + typed SVG | First large JSON-render migration; keep parity gates tight while splitting renderer modules. |
| `zenuml` | `json-fallback` | JSON layout + JSON SVG via sequence renderer | Shares the sequence renderer but still enters through semantic JSON. |
| `gantt` | `typed-first` | typed layout + typed SVG | Migrated after kanban; keep date/timezone parity gates tight. |
| `kanban` | `typed-first` | typed layout + layout-only SVG | Migrated after sequence; keep as the small-diagram typed model pattern. |
| `er`, `erDiagram` | `json-fallback` | JSON layout + JSON SVG | Lower priority than sequence/gantt/kanban; mature parity path. |
| `block` | `json-fallback` | JSON layout + JSON SVG | Defer unless profiling shows JSON cost. |
| `requirement` | `typed-first` | typed layout + typed SVG | Requirements/relations/classes now share the core render model. |
| `radar` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `treemap` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `info` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `packet` | `typed-first` | typed layout + typed SVG | Config-heavy small diagram; keep as a high-ROI migration pattern. |
| `timeline` | `typed-first` | typed layout + typed SVG | Moderate small-diagram migration; watch layout/render midpoint drift. |
| `journey` | `typed-first` | typed layout + typed SVG | Small-to-moderate migration; watch render midpoint drift. |
| `gitGraph` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `pie` | `typed-first` | typed layout + typed SVG | Small typed migration; keep as a simple-diagram pattern. |
| `xychart` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `quadrantChart` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `sankey` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `c4` | `json-fallback` | JSON layout + JSON SVG | Defer. |
| `error` | `json-fallback` | JSON layout + JSON SVG | Keep JSON; this is the fallback payload for suppressed parse errors. |

## API Decision

`parse_diagram_for_render_sync` is obsolete. It predates `ParsedDiagramRender` and only has
special JSON-for-render handling for `mindmap` and `stateDiagram`, both of which now have typed
render models. The in-tree render helper already uses `parse_diagram_for_render_model_sync`.

Decision:

- Removed `parse_diagram_for_render_sync` and its async alias from `merman-core`.
- Removed `parse_mindmap_for_render` and `parse_state_for_render`.
- Keep `parse_diagram_sync` as the stable semantic JSON API.
- Keep `parse_diagram_for_render_model_sync` as the render-pipeline API.
- Keep `layout_diagram_sync` on semantic JSON for now because it returns `LayoutedDiagram` with a
  JSON semantic payload; revisit this only after the public render API is reviewed.
