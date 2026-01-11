# ADR-0015: Sequence Diagram Parser Technology

## Status

Accepted

## Context

`sequenceDiagram` is implemented in Mermaid via a Jison parser (`sequenceDiagram.jison`) using:

- case-insensitive lexing,
- lexer modes (e.g. ID/ALIAS/LINE),
- a fixed set of arrow tokens mapped to numeric `LINETYPE` constants,
- many user-visible edge cases (wrap prefixes, comments, mixed separators).

`merman` must preserve Mermaid's user-facing behavior and error surfaces for compatibility.

## Decision

- Implement `sequenceDiagram` using the same approach as flowchart:
  - a deterministic, state-machine lexer (mode switching where required),
  - an LALR(1) grammar implemented via `lalrpop`,
  - a DB-like semantic model builder aligned to Mermaid `SequenceDB` outputs (headless).
- Drive parity using upstream specs (`repo-ref/mermaid/.../sequenceDiagram.spec.js`) and keep a
  living alignment document (`docs/alignment/SEQUENCE_MINIMUM.md`) for incremental slices.

## Consequences

Pros:
- Closest match to upstream Jison behavior (especially token boundaries and precedence).
- Incremental expansion without changing the crate public API.

Cons:
- Higher initial implementation effort than ad-hoc line parsing.
- Requires careful testing around lexer precedence and comment handling to reach full parity.

