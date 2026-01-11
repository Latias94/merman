# ER Diagram Minimum (mermaid@11.12.2)

This document tracks the current `erDiagram` / `er` parser alignment status in `merman-core`.

Upstream references:

- Parser: `repo-ref/mermaid/packages/mermaid/src/diagrams/er/parser/erDiagram.jison`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/er/erDb.ts`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/er/parser/erDiagram.spec.js`

## Implemented (phase 1)

- Type detection: `er` when input starts with `erDiagram`.
- Stand-alone entities:
  - `erDiagram\nISLAND\nMAINLAND`
- Validation (minimal):
  - `""` (empty quoted entity name) is rejected.
  - Quoted entity names containing `%` or `\\` are rejected (matches upstream constraints).
- Relationships (identifying vs non-identifying):
  - `||--o{`, `||..o{`, `|o--|{`, `}|--||`, etc.
  - `||--|{` (ONE_OR_MORE marker)
  - Role parsing after `:` via quoted string or bare identifier.
  - Numeric shorthands: `1+`, `0+`, `1`.
  - Supports `..`, `.-`, `-.` non-identifying variants.
  - Supports word aliases from upstream spec (e.g. `one or zero`, `zero or many`, `many(0)`, `optionally to`).
- Entity alias:
  - `foo["bar"]`
- Attribute blocks:
  - `BOOK { string title PK, FK "comment" }`
  - Attribute names can contain `-`, `[ ]`, `( )` (e.g. `author-ref[name](1)`).
  - Supports generic-ish tokens (e.g. `type~T~`) and common type shapes like `string[]`, `varchar(5)`.
  - Key list supports `PK`, `FK`, `UK` with commas and whitespace.
  - Comment uses double quotes.
  - Empty blocks are accepted: `BOOK {}` / `BOOK{}`.
  - Multiple blocks for the same entity append attributes (matches Mermaid behavior).
- Styling statements:
  - `style <ids> <css...>` → appends `cssStyles` to entities.
  - `classDef <classes> <css...>` → stores class definitions in `classes`.
  - `class <entities> <classes>` → appends to entity `cssClasses`.
  - Inline `:::<classes>` after entity name in declarations and relationships.
  - Style/classDef whitespace inside definitions is ignored (matches upstream style-lexer behavior).
- Accessibility:
  - `accTitle: ...`
  - `accDescr: ...`
  - `accDescr { ... }` (multiline, de-indented on continuation lines).
- Direction statement:
  - `direction TB|BT|LR|RL`

## Output shape (current)

The parser returns a headless semantic model:

- `entities`: map keyed by entity name.
- `relationships`: array referencing entity DOM ids (`entity-<name>-<n>`) like Mermaid.
- `classes`: class definitions (`styles`, `textStyles`).

Note: Mermaid renderers typically consume `ErDB.getData()` (layout-friendly nodes/edges). A future
crate can convert this model into layout data for rendering.

## Known gaps (to be closed)

- Full upstream lexical strictness (edge-case validation) is still being iterated.
- Complete coverage of all relationship/cardinality spellings from the upstream spec suite.
- Diagnostics alignment (error messages and offsets).
