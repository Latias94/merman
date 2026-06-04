# EventModeling Minimum (Mermaid@11.15.0)

This document tracks the first local support slice for Mermaid `eventmodeling`.

Upstream references at locked commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- Detector: `packages/mermaid/src/diagrams/eventmodeling/detector.ts`
- DB/model: `packages/mermaid/src/diagrams/eventmodeling/db.ts`
- Parser adapter: `packages/mermaid/src/diagrams/eventmodeling/parser.ts`
- Renderer: `packages/mermaid/src/diagrams/eventmodeling/renderer.ts`
- Styles: `packages/mermaid/src/diagrams/eventmodeling/styles.js`
- Types: `packages/mermaid/src/diagrams/eventmodeling/types.ts`
- Grammar: `packages/parser/src/language/eventmodeling/event-modeling.langium`
- Syntax docs: `docs/syntax/eventmodeling.md`

## Implemented (Phase 1)

- Detection:
  - accepts `eventmodeling`
  - exposes internal diagram id `eventmodeling`, matching upstream detector id
- Parser:
  - supports `tf` / `timeframe` and `rf` / `resetframe`
  - captures frame name, entity type, qualified entity identifier, explicit `->>` sources, inline data, and `[[dataReference]]`
  - captures `data` blocks as named data entities
  - ignores blank lines and whole-line `%%` comments
- Render model:
  - typed `EventModelingDiagramRenderModel`
  - compatibility JSON from the same typed model
- Layout:
  - ports the upstream swimlane, box sizing, overlap, relation endpoint, and entity color defaults
  - supports `eventmodeling.padding`, `eventmodeling.useMaxWidth`, and eventmodeling theme variables
  - preserves local swimlane namespace state for stable lane reuse
- SVG:
  - Stage B renderer with `.eventmodeling`, `.eventModeling-swimlane`, `.eventModeling-box`, `.eventModeling-reset-box`, `.eventModeling-relation`, and arrow marker DOM signals
  - renders reset frames with dashed boxes
  - uses frame fill/stroke colors and `themeVariables.emRelationStroke`

## Known Gaps

- No dedicated `xtask compare-eventmodeling-svgs` command yet.
- No committed upstream SVG baseline corpus yet.
- `entity`, `note`, and `gwt` statements are not rendered in Phase 1.
- Data block formatting is captured conservatively and rendered as plain text rather than upstream `foreignObject` HTML.
- Local namespace swimlane reuse is more stable than the current upstream runtime state shape, so strict layout parity has not been claimed.
- Browser `foreignObject`, HTML sanitization, and `getBBox()` float parity have not been strict-audited.
