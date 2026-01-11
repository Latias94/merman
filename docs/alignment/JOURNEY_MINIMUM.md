# Journey Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for journey parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `journey` (case-insensitive).
- Statements (case-insensitive):
  - `title <text>` (single line; stops at `#` or `;` like the Jison lexer).
  - `section <name>` (single line; stops at `#`, `:` or `;` like the Jison lexer).
  - accessibility:
    - `accTitle: ...` (single-line; stops at `#` or `;`)
    - `accDescr: ...` (single-line; stops at `#` or `;`)
    - `accDescr { ... }` (multi-line, ends at `}`)
- Tasks:
  - Line form: `<taskName>: <score>[: <comma separated actors>]`
  - Actor lists are split on commas and trimmed; empty actor entries are preserved (e.g. `5:` -> `[""]`), matching Mermaid.
  - `score` is parsed as a number; the Mermaid docs specify a range of 1..=5.
- Comments:
  - `# ...` comment lines are ignored (mirrors `\#[^\n]*`).
  - `%% ...` comment lines are ignored (mirrors Mermaid’s `%%` comment convention).

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s journey DB state:
  - `type`, `title`, `accTitle`, `accDescr`
  - `sections: string[]`
  - `tasks: { score, people[], section, type, task }[]`
  - `actors: string[]` (unique, sorted)

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `journey` grammar and DB behavior
compatibility at the pinned baseline tag.

