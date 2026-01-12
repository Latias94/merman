# Info Diagram Minimum (mermaid@11.12.2)

This document tracks the current `info` parser alignment status in `merman-core`.

Upstream references:

- Parser: `repo-ref/mermaid/packages/mermaid/src/diagrams/info/infoParser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/info/infoDb.ts`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/info/info.spec.ts`

## Implemented (phase 1)

- Header:
  - `info`
  - `info showInfo`
- Unsupported grammar:
  - extra tokens are rejected with an upstream-matching error string for the covered cases.
- Comments:
  - `%% ...` lines are ignored.

## Output shape (current)

The parser returns a minimal headless model:

- `showInfo`: boolean

## Known gaps (to be closed)

- Full error offset parity for multi-line inputs and other invalid tokens.

