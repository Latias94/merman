# Requirement Diagram Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Requirement Diagram parsing in
`merman`.

Baseline: Mermaid `@11.12.2`.

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

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s Requirement DB behavior:
  - `type`
  - `accTitle`, `accDescr`
  - `direction`
  - `requirements`: array of `{ name, type, requirementId, text, risk, verifyMethod, cssStyles, classes }`
  - `elements`: array of `{ name, type, docRef, cssStyles, classes }`
  - `relationships`: array of `{ type, src, dst }`
  - `classes`: map of `{ id, styles, textStyles }`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `requirement` grammar and DB
behavior compatibility at the pinned baseline tag.

