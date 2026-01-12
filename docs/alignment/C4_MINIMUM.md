# C4 Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for C4 parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Headers:
  - `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment`
- Common metadata:
  - `title ...`
  - `accDescription ...`
  - `accDescr: ...` and multiline `accDescr { ... }`
  - `accTitle: ...` is treated as `title` (upstream grammar quirk)
- Direction:
  - `direction TB|BT|LR|RL` is accepted (no-op)
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

## Argument parsing rules (current)

- Arguments are comma-separated inside `(...)`.
- Empty arguments are allowed and become empty strings.
- Quoted strings use `"..."`
  - No escape processing (aligning with Mermaid’s C4 lexer behavior).
- Key/value attributes are supported:
  - `$key="value"` becomes `{ "key": "value" }`

## Output shape (Phase 1)

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

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid C4 grammar and DB behavior
compatibility at the pinned baseline tag.

## Notes on DB behavior

- Relationships are de-duplicated by `(from,to)` and later statements override earlier ones,
  matching Mermaid’s `c4Db.js` behavior.
