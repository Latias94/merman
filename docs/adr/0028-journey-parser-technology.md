# ADR 0028: Journey Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-11
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s journey diagram implementation is split between:

- A Jison grammar (`packages/mermaid/src/diagrams/user-journey/parser/journey.jison`) that parses:
  - `title`, `section`, accessibility fields
  - task rows in the form `Task name: <score>[: <actors>]`
  - line comments (`# ...`) and Mermaid comments (`%% ...`).
- A small stateful DB (`packages/mermaid/src/diagrams/user-journey/journeyDb.js`) that stores:
  - `sections[]`
  - `tasks[]` with `{ score, people, section, type, task }`
  - `actors[]` derived from task `people`, unique + sorted.

`merman` must provide a headless, 1:1 compatible parser where upstream behavior is the spec.

## Decision

Implement journey parsing in `merman-core` as a hand-written, line-oriented parser that mirrors
the Jison grammar’s observable behavior:

- Require `journey` header.
- Parse `title` and `section` statements with Mermaid’s token termination rules (`#` / `;` and `:` where applicable).
- Parse tasks as `taskName: taskData` and apply Mermaid’s DB splitting rules:
  - split on `:` to separate score and actor list
  - split actors on `,` and trim, preserving empty entries.
- Compute `actors` as a unique sorted list from task `people`.
- Defer HTML sanitization to the shared `commonDb`-aligned post-processing step (ADR-0020).

Rationale: journey is line-driven and DB-like; a dedicated parser is small, easy to align with
Mermaid’s own test vectors, and avoids introducing unnecessary parsing infrastructure.

## Consequences

- Journey parity is primarily maintained by porting Mermaid’s upstream tests:
  - `journeyDb.spec.js`
  - `parser/journey.spec.js`
- The headless model exposes the DB snapshot required by renderer integration layers.

## Revisit criteria

Reconsider if Mermaid migrates journey parsing to a shared parser framework (e.g. `@mermaid-js/parser`)
or significantly expands journey syntax beyond its current line-driven structure.

