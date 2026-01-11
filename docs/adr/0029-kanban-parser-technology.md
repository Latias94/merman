# ADR 0029: Kanban Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-11
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s kanban diagram implementation consists of:

- A Jison grammar (`packages/mermaid/src/diagrams/kanban/parser/kanban.jison`) that parses:
  - `kanban` header
  - indentation-based node levels (similar to mindmap)
  - node shapes via bracket/paren delimiters
  - decorations (`::icon(...)`, `:::class`)
  - `@{ ... }` metadata blocks with special newline handling inside double-quoted strings
- A stateful DB (`packages/mermaid/src/diagrams/kanban/kanbanDb.ts`) that:
  - classifies nodes into sections (columns) and items
  - enforces the “items must belong to a section” invariant via `getSection(level)`
  - parses metadata via `js-yaml` JSON schema and applies specific validation/mapping rules
  - exposes `getSections()` and `getData()` as the renderer-facing API

`merman` must provide a headless, 1:1 compatible parser where upstream behavior is the spec.

## Decision

Implement kanban parsing in `merman-core` as a hand-written, line-oriented parser that mirrors the
observable behavior of Mermaid’s Jison + DB pair:

- Detect and require a `kanban` header (case-insensitive).
- Parse each statement line with:
  - indentation (`level`) computed from leading whitespace length
  - node parsing compatible with Mermaid’s delimiter rules
  - decoration statements that apply to the most recently parsed node
  - inline `%% ...` comment stripping outside quotes
  - `@{ ... }` metadata extraction with Mermaid’s newline-to-`<br/>` rewrite within double quotes
- Maintain a DB-like in-memory model that reproduces Mermaid’s `getSection(level)` rules and error
  messages.
- Parse metadata using `serde_yaml` and map fields to the node in the same way as Mermaid’s DB.
- Expose a semantic output aligned with Mermaid’s `kanbanDb.getData()` (and `getSections()` for parity tests).

Rationale: kanban is indentation-driven and DB-like; a dedicated parser keeps alignment work local,
porting upstream tests is straightforward, and it avoids introducing additional parsing frameworks.

## Consequences

- Parity is maintained primarily by porting Mermaid’s upstream tests (`kanban.spec.ts`).
- The headless output can be consumed by future renderer crates (SVG/CLI/UI integrations) without
  embedding a JS runtime.

## Revisit criteria

Reconsider if Mermaid migrates kanban parsing to a shared parser framework (e.g. `@mermaid-js/parser`)
or significantly expands the kanban syntax beyond indentation + metadata.

