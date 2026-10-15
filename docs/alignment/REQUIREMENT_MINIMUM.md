# Requirement Diagram Admission Contract

This document defines the admitted Requirement parser, model, Dagre layout, and SVG contract.

Historical parser admission baseline: Mermaid `11.16.1` at
`7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`. The current reference is the pinned Mermaid
`11.17.2` checkout specified by ADR-0001. The SVG-label contract below was checked against
that checkout; this does not relabel historical fixtures as regenerated evidence.

Upstream references:

- Parser grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/requirement/parser/requirementDiagram.jison`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/requirement/parser/requirementDiagram.spec.js`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/requirement/requirementDb.ts`
- DB tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/requirement/requirementDb.spec.ts`

## Supported (current)

- Header:
  - `requirementDiagram` (case-insensitive).
- Common metadata:
  - `accTitle: ...`
  - `accDescr: ...`
  - Multiline `accDescr { ... }` (trimmed; preserves internal newlines).
- Direction:
  - `direction TB | BT | LR | RL`
- Requirement blocks:
  - Types:
    - `requirement`
    - `functionalRequirement`
    - `interfaceRequirement`
    - `performanceRequirement`
    - `physicalRequirement`
    - `designConstraint`
  - Optional shorthand classes on definition: `<name>:::class1,class2`
  - Body keys:
    - `id: ...`
    - `text: ...`
    - `risk: low|medium|high` (case-insensitive; stored as `Low|Medium|High`)
    - `verifyMethod: analysis|demonstration|inspection|test` (case-insensitive; stored as title case)
- Element blocks:
  - `element <name> [:::classList] { ... }`
  - Body keys:
    - `type: ...`
    - `docref: ...`
- Relationships:
  - `<id> - <relationship> -> <id>`
  - `<id> <- <relationship> - <id>`
  - Supported relationships:
    - `contains`, `copies`, `derives`, `satisfies`, `verifies`, `refines`, `traces`
- Styles and classes:
  - `style <idList> <style1,style2,...>`
  - `classDef <classList> <style1,style2,...>`
  - `class <idList> <classList>`
  - Shorthand assignment: `<id>:::class1,class2`
  - Class application inherits `classDef` styles into node `cssStyles` (no deduplication).
- Comments:
  - `# ...` and `%% ...` are treated as comments for regular statements.
  - `style` / `classDef` / `class` statements do not treat `#` as a comment marker (needed for
    hex colors like `#f9f`), aligning with Mermaid’s lexer state.

## Output Shape

- The semantic output is a headless snapshot aligned with Mermaid’s Requirement DB behavior:
  - `type`
  - `accTitle`, `accDescr`
  - `direction`
  - `requirements`: array of `{ name, type, requirementId, text, risk, verifyMethod, cssStyles, classes }`
  - `elements`: array of `{ name, type, docRef, cssStyles, classes }`
  - `relationships`: array of `{ type, src, dst }`
  - `classes`: map of `{ id, styles, textStyles }`
  - `config`

## Layout And SVG Admission

- Requirement and element lines honor root `htmlLabels` (default `true`). Relationships and
  self-loop anchors follow the shared effective setting, including the `flowchart.htmlLabels`
  fallback when the root setting is absent, matching the distinct upstream consumers.
- With `htmlLabels: false`, these labels emit SVG `text`/`tspan`, retain Markdown and decoded
  entities, and use SVG-mode measurement. Node stacking includes the upstream 6px bbox-height
  addition; relationship backgrounds instead add 2px on each side of their text bounds.
- Source label `color` becomes `fill` only on SVG `text`; the label group and first-row inner
  spans retain their source declarations. Shape fill does not become text fill. The name line
  retains the upstream first-row bold override; ELK-configured body lines retain start alignment.
- Prepared-label reuse binds both node and relationship modes in addition to typography and
  measurement identity. Host measurers retain render-time callbacks. Browser glyph overhang and
  bbox offsets remain text-measurement residuals, not pixel-exact qualification.
- Relationship ownership uses a structured edge key, so duplicate sources, reverse edges, and
  self-loops do not bind labels by source text or array position.
- Dagre self-loops preserve Mermaid's helper segments, marker ids, and route restoration.
- Updated paths are finalized before label projection; fixed-width labels retain Mermaid's `200px`
  wrapping contract.
- `upstream_cypress_requirementdiagram_unified_spec_example_003` is the signed `<<traces>>`
  semantic-label canary. Its browser text-measurement residuals are exact catalog entries, not a
  family-wide coordinate tolerance.
