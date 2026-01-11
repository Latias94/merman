# Timeline Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for timeline parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `timeline` (case-insensitive).
- Statements:
  - `title <text>` (entire remainder of the line is the title, including `#` and `;`).
  - `section <name>` (entire remainder of the line up to an optional `:` is the section name).
  - accessibility (mirrors Mermaid `commonDb.ts`):
    - `accTitle: ...` (single-line; stops at `#` or `;`)
    - `accDescr: ...` (single-line; stops at `#` or `;`)
    - `accDescr { ... }` (multi-line, ends at `}`)
- Tasks and events:
  - A task line is a “period” statement: `<taskText>`
  - Events are introduced via `: <eventText>` and are attached to the most recent task.
  - Multiple events can be placed on a single physical line: `task: ev1: ev2`
  - Multi-line events can be continued by starting the next line with `: ...`
  - Event splitting matches the Jison lexer: a new event starts at `:` followed by whitespace (`:\s`), so `http://...` stays within a single event.
- Comments:
  - full-line `# ...` comments are ignored (mirrors `\#[^\n]*` in the lexer).

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s timeline DB state:
  - `type`, `title`, `accTitle`, `accDescr`
  - `sections: string[]`
  - `tasks: { id, section, type, task, score, events[] }[]`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `timeline` grammar and DB behavior
compatibility at the pinned baseline tag.

