# ADR 0039: Requirement Diagram Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `requirement` diagram is implemented using a Jison grammar plus a stateful DB:

- Grammar: `packages/mermaid/src/diagrams/requirement/parser/requirementDiagram.jison`
  - header token: `requirementDiagram` (case-insensitive)
  - statements:
    - `accTitle: ...`
    - `accDescr: ...` and multiline `accDescr { ... }`
    - `direction <TB|BT|LR|RL>`
    - requirement blocks: `<requirementType> <name> [:::classList] { ... }`
    - element blocks: `element <name> [:::classList] { ... }`
    - relationships: `<id> - <rel> -> <id>` and `<id> <- <rel> - <id>`
    - style/class: `style ...`, `classDef ...`, `class ...`, and shorthand `id:::classList`
- DB behavior: `packages/mermaid/src/diagrams/requirement/requirementDb.ts`
  - requirements/elements default class: `default`
  - `classDef` stores `styles` and derives `textStyles` when a style contains `color`
  - `class` / shorthand class application appends classes and inherits class styles into node styles

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `requirement` parsing in `merman-core` as a handwritten, line-oriented parser plus a
DB-like state model:

- Keep input handling close to Mermaid’s lexer states:
  - `#` and `%%` are treated as comments for regular statements.
  - `style` / `classDef` / `class` statements must not treat `#` as a comment marker (needed for
    hex colors like `#f9f`).
- Preserve Mermaid’s DB semantics:
  - requirements/elements get `classes: ["default"]` at creation
  - applying classes can duplicate entries (no automatic deduplication), mirroring Mermaid

## Rationale

- The grammar is statement-oriented and relatively small; a dedicated Rust parser is faster to
  iterate for parity than introducing a full lexer/parser generator toolchain at this stage.
- Parity is enforced by porting Mermaid’s parser/DB behavior tests into Rust unit tests.

## Consequences

- The current headless model focuses on semantic state (requirements/elements/relationships, plus
  class/style resolution) and intentionally does not include rendering concerns.
- If Mermaid expands the grammar to require a true token stream (e.g., to support escaping rules
  in more places), revisit this decision and consider a shared lexer abstraction.

