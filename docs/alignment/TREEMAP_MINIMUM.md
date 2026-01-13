# Treemap Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Treemap parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Parser/AST bridge: `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/parser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/db.ts`
- Hierarchy builder: `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/utils.ts`
- Hierarchy tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/utils.test.ts`

## Supported (current)

- Header:
  - `treemap`
  - `treemap-beta`
- Common metadata:
  - `title ...`
  - `accTitle: ...`
  - `accDescr: ...` and `accDescr{...}` (single-line)
  - Last assignment wins.
- Rows (indentation-driven):
  - Indentation uses leading spaces/tabs; the hierarchy key is the character count (mirrors
    Mermaid’s Langium value converter for `INDENTATION`).
  - Section row:
    - `"Section Name"`
    - Optional class selector: `"Section Name":::className`
  - Leaf row:
    - `"Leaf Name": 12` (also supports comma separator `"Leaf Name", 12`)
    - Optional class selector: `"Leaf Name": 12:::className`
  - Names are simple quoted strings without escapes (matches `STRING2` terminal).
  - Values accept digits, commas and decimals (mirrors Mermaid’s `NUMBER2` + `parseFloat`-style
    conversion).
- Styling:
  - `classDef className <styleText>;` is supported.
  - Style splitting matches Mermaid:
    - `\,` escapes commas inside values
    - `,` is treated like `;` as a separator
    - styles are stored in `classes[className].styles` and compiled into
      `cssCompiledStyles: ["...;..."]` on nodes.

## Output shape (Phase 1)

- Headless semantic output aligned with Mermaid’s Treemap DB behavior:
  - `type`
  - `title`, `accTitle`, `accDescr`
  - `root`: `{ name: "", children: [...] }`
  - `nodes`: preorder flattened nodes, each with `level` (recursion depth)
  - `classes`: class style definitions
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `treemap` grammar and DB behavior
compatibility at the pinned baseline tag.
