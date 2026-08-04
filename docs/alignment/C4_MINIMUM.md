# C4 Parser And Semantic Contract

This document defines the admitted C4 parser and semantic-model contract in `merman`.

Baseline: Mermaid `11.16.0` at `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

## Supported (current)

- Headers:
  - `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment`
- Common metadata:
  - `title ...`
  - `accDescription ...`
  - `accDescr: ...` and multiline `accDescr { ... }`
  - `accTitle: ...` is treated as `title` (upstream grammar quirk)
- Direction:
  - Mermaid 11.16 rejects `direction TB|BT|LR|RL` in the C4 render parser.
  - Editor recovery retains parser-backed facts for the rejected line without constructing a
    render model from it.
- Macros (subset; consistent parsing rules):
  - People / systems:
    - `Person`, `Person_Ext`
    - `System`, `SystemDb`, `SystemQueue`
    - `System_Ext`, `SystemDb_Ext`, `SystemQueue_Ext`
  - Containers:
    - `Container`, `ContainerDb`, `ContainerQueue`
    - `Container_Ext`, `ContainerDb_Ext`, `ContainerQueue_Ext`
  - Components:
    - `Component`, `ComponentDb`, `ComponentQueue`
    - `Component_Ext`, `ComponentDb_Ext`, `ComponentQueue_Ext`
  - Boundaries (nested blocks):
    - `Boundary(...) { ... }`
    - `Enterprise_Boundary(...) { ... }`
    - `System_Boundary(...) { ... }`
    - `Container_Boundary(...) { ... }`
  - Deployment nodes (nested blocks):
    - `Node(...) { ... }`, `Node_L(...) { ... }`, `Node_R(...) { ... }`
    - `Deployment_Node(...) { ... }` (alias of `Node`)
- Relationships:
    - `Rel`, `BiRel`, `Rel_Up/Rel_U`, `Rel_Down/Rel_D`, `Rel_Left/Rel_L`, `Rel_Right/Rel_R`, `Rel_Back`
    - `RelIndex(index, ...)` (index is ignored, matching Mermaid’s parser splice)
  - Style / layout updates:
    - `UpdateElementStyle(...)`
    - `UpdateRelStyle(...)`
    - `UpdateLayoutConfig(...)` (enforces `>= 1`)

## Argument Semantics

- Arguments are comma-separated inside `(...)`.
- Empty arguments are allowed and become empty strings.
- Quoted strings use `"..."`
  - No escape processing (aligning with Mermaid’s C4 lexer behavior).
- Arguments are represented internally as ordered `C4Arg::Text` and `C4Arg::Named` values. JSON
  objects are a compatibility projection, not the parser's semantic carrier.
- Named attributes retain source order, duplicates, unknown keys, and sparse positional slots.
  Structural fields such as aliases cannot be overwritten through named attributes.
- `UpdateRelStyle` resolves named fields independently of order and uses JavaScript `parseInt`
  behavior for offsets. The historical `offsetX`/`offsetY` positional rebinding is not retained.
- If a named object occupies a positional `label`, `type`, `techn`, or `descr` slot, the DB keeps
  Mermaid's nested text-object side effect. Dedicated sprite/tags/link positions remain supported.

## Output Shape

- Headless semantic snapshot:
  - `type` (always `c4`)
  - `c4Type` (the specific header token)
  - `title`, `accTitle`, `accDescr`
  - `wrap`
  - `layout`: `{ c4ShapeInRow, c4BoundaryInRow }`
  - `shapes`: array of shape objects (each has at least `alias`, `label`, `descr`, `typeC4Shape`, `parentBoundary`, `wrap`)
  - `boundaries`: array of boundary objects (includes the implicit `global` boundary)
  - `rels`: array of relationship objects
  - `config`

## Structural Semantics

- Boundary bodies are transactional: malformed, empty, unmatched, or unclosed bodies do not leak
  declarations or parent state into the render model.
- Shape and relation redeclaration replaces omitted optional fields instead of retaining stale
  values. Relationship lookup keeps Mermaid's first-match identity semantics.
- SVG paint order follows each boundary subtree rather than grouping all boundaries before all
  shapes. Relationship offsets, line styles, and text styles remain attached to their ordered
  relation identity.
- The semantic edge-label gate and browser computed-style gate are documented in
  `SEMANTIC_LABEL_PARITY.md` and `C4_LAYOUT_UPSTREAM_TEST_COVERAGE.md`.

## Notes on DB behavior

- Relationships are de-duplicated by `(from,to)` and later statements override earlier ones,
  matching Mermaid’s `c4Db.js` behavior.
- `Enterprise_Boundary` / `System_Boundary` / `Container_Boundary` inject a fixed `type` (respectively
  `ENTERPRISE` / `SYSTEM` / `CONTAINER`) into the boundary object, matching Mermaid’s grammar splice.
- Deployment nodes ignore the `sprite` argument (`c4Db.js` accepts it but does not store it).
- `accDescr` is subject to Mermaid common DB sanitization (`\n\s+` is collapsed to `\n`).
