# Config and Frontmatter Support Matrix

This document tracks Mermaid config/frontmatter behavior that is intentionally supported by
`merman`, plus known gaps where config is preserved but not fully consumed by renderers.

## Merge Semantics

| Input source | Status | Notes |
| --- | --- | --- |
| Engine site config | Supported | Loaded from generated upstream defaults, then user site config is deep-merged. |
| Frontmatter `config` | Supported | Parsed as Mermaid config overrides. |
| Frontmatter `title` | Supported | Sanitized with the effective config before metadata is returned. |
| Frontmatter `displayMode` | Supported | Mermaid special case mapped to `gantt.displayMode`. |
| Frontmatter top-level diagram namespaces | Supported compatibility layer | Known diagram namespaces such as `gantt`, `flowchart`, `class`, `er`, `state`, and `xyChart` are mapped into `config.<diagram>`. Explicit `config` values take priority. |
| Frontmatter arbitrary top-level YAML fields | Not supported | Unknown keys are ignored, matching the narrow upstream frontmatter surface. |
| `%%{init: ...}%%` directives | Supported | Directive config is merged after frontmatter, so directive values win. |
| Directive top-level `config` field | Supported | Mirrors Mermaid's behavior by moving directive `config` into the detected diagram-specific namespace. |

## Config Feature Matrix

| Config area | Parse/preprocess status | Render/behavior status | Notes |
| --- | --- | --- | --- |
| `theme` | Supported | Supported | Theme defaults are applied to the effective config before parsing/rendering. |
| `themeVariables` | Supported | Supported | Includes legacy `fontFamily` mirroring into `themeVariables.fontFamily`. |
| `themeCSS` | Supported | Supported | SVG output scopes injected CSS through the parity CSS postprocessor. |
| `look` | Supported | Partial by diagram | Flowchart, state, mindmap, requirement, class, ER, and Kanban section clusters consume `look` in SVG DOM/style paths. Sequence consumes `look` in presentation CSS/theme paths, but does not currently expose a diagram-wide `data-look` DOM contract. Kanban item groups do not currently expose a broader `data-look` DOM contract. |
| `layout` | Supported | Partial by diagram | Detection preserves `flowchart-elk` / `flowchart.defaultRenderer=elk` side effects, but the local flowchart layout path is not a full ELK implementation. |
| `flowchart.defaultRenderer` | Supported | Partial | Detection can select `flowchart-elk` semantics and set root `layout=elk`; layout parity remains incomplete. |
| `class.defaultRenderer` | Supported | Supported for detector branching | Used to select class renderer variants like Mermaid's detector order. |
| `state.defaultRenderer` | Supported | Supported for detector branching | Used to select state renderer variants like Mermaid's detector order. |
| `gantt.displayMode` | Supported | Supported | Frontmatter `displayMode` and `config.gantt.displayMode` both reach Gantt parse/render paths. |
| `gantt.useWidth` | Supported | Supported | Consumed by Gantt SVG layout. |
| `gantt.rightPadding` | Supported | Supported | Consumed by Gantt SVG layout. |
| `gantt.topAxis` | Supported | Supported | Consumed by Gantt SVG layout. |
| `gantt.numberSectionStyles` | Supported | Supported | Consumed by Gantt SVG layout. |

## Known Gaps

- `layout: elk` is not a full local ELK layout implementation for flowcharts yet. Treat current
  support as detection/config plumbing, not layout parity.
- `look` is not a universal all-diagram contract. Renderers should only claim support after tests
  verify both effective config propagation and rendered SVG/CSS consumption.
- Top-level frontmatter compatibility is intentionally narrow. Global Mermaid config keys such as
  `theme`, `look`, and `layout` should still be written under `config`.
