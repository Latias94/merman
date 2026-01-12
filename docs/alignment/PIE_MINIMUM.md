# Pie Diagram Minimum (mermaid@11.12.2)

This document tracks the current `pie` parser alignment status in `merman-core`.

Upstream references:

- Parser: `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieParser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieDb.ts`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pie.spec.ts`

## Implemented (phase 1)

- Header:
  - `pie`
  - `pie showData`
  - `pie title <text...>`
  - `pie showData title <text...>` (order-insensitive support is allowed)
- Sections:
  - `"label": <number>` / `'label': <number>`
  - Duplicate labels keep the first value (matching upstream `Map` semantics).
  - Negative values are rejected with Mermaid's error message.
- Common fields:
  - `accTitle: ...`
  - `accDescr: ...`
  - Multiline `accDescr { ... }` blocks (trimmed lines joined with `\n`).
- Comments:
  - `%% ...` lines are ignored.

## Output shape (current)

The parser returns a headless semantic model:

- `showData`: boolean
- `title`: optional string
- `accTitle` / `accDescr`: optional strings
- `sections`: array of `{ label, value }` preserving insertion order

## Known gaps (to be closed)

- Error message and offset parity beyond the covered negative-value cases.
- Full grammar parity with `@mermaid-js/parser` (once we implement a shared lexer/parser pipeline across diagrams).

