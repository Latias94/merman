# Architecture Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for architecture parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Parser/AST bridge: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureParser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureDb.ts`
- Upstream tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architecture.spec.ts`

## Supported (current)

- Header:
  - `architecture-beta`
  - Allows empty lines above the header.
  - Allows `title ...` directly on the header line: `architecture-beta title sample title`
- Title and accessibility (common parser terminals):
  - `title ...` (stops at `%%` comment)
  - `accTitle: ...` (stops at `%%` comment)
  - `accDescr: ...` (stops at `%%` comment)
  - `accDescr { ... }` multi-line block, ends at first `}`
- Statements:
  - `group <id>(<icon>)?[<title>]?( in <parent>)?`
  - `service <id>(<icon>)|<quoted iconText>?[<title>]?( in <parent>)?`
  - `junction <id>( in <parent>)?`
  - Edge (colon form):
    - `<lhsId>{group}?:<L|R|T|B> <|>? (-- | -[Title]-) <|>? <L|R|T|B>:<rhsId>{group}?`
- Inline comments:
  - Trailing `%% ...` is ignored unless inside quotes.

## Output shape (Phase 1)

- `type`, `title`, `accTitle`, `accDescr`
- `groups[]`, `nodes[]`, `services[]`, `junctions[]`, `edges[]`
- `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `architecture` grammar and DB behavior
compatibility at the pinned baseline tag.
