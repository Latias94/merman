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

- Use a deterministic state-machine lexer and a handwritten predictive parser in `merman-core`.
- Track open fragments and boxes on a heap stack. Append actions in semantic replay order and
  pass the completed list to the builder aligned with Mermaid `SequenceDB`.
- Drive parity using upstream specs (`repo-ref/mermaid/.../sequenceDiagram.spec.js`) and the
  [family compatibility notes](../alignment/FAMILY_NOTES.md#sequence-and-timeline).

## Consequences

- The lexer owns token boundaries, including mode transitions and comment handling.
- The parser owns block and statement boundaries, expected tokens, and action order.
- Cancellation is checked during lexical scanning, token delivery, action emission, and replay.
- Editor recovery can inspect the full lexical event tape and reuse the parser's structured
  diagnostic.
