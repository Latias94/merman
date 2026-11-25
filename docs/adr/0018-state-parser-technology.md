# ADR-0018: State Parser Technology

## Status

Accepted

## Context

Mermaid `stateDiagram` (including `stateDiagram-v2`) is defined by a Jison lexer+parser in the
upstream baseline (`repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/stateDiagram.jison`)
and populated into `StateDB` (`repo-ref/mermaid/packages/mermaid/src/diagrams/state/stateDb.ts`).

We need:

- Deterministic, testable parsing behavior aligned to Mermaid `@11.12.3`.
- A headless semantic model suitable for renderer integration in other crates (SVG, CLI, GUI).
- A maintainable approach consistent with other diagram parsers in `merman-core`.

## Decision

- Implement `stateDiagram` parsing in `merman-core` using:
  - A small stateful handwritten lexer in Rust (mirroring Jison start conditions where relevant).
  - A handwritten predictive parser over spanned lexer events. It uses an explicit stack for open
    composites and adds their statement lists to the document arena in closing order.
  - An in-memory DB (`StateDb`) that:
    - Applies Mermaid v2 start/end rewriting (`[*]` -> `root_start`/`root_end`, nested via parent id).
    - Applies class/style statements (`classDef`, `class`, `style`) to states.
    - Produces a headless semantic model.

## Consequences

- The shared token cursor carries lexer spans and checks cancellation as it delivers tokens.
- The parser owns expected tokens for each syntax boundary, including the end of a composite.
- The editor reads the same lexical event tape and can recover symbols after a syntax error.
- Source fixtures and malformed token sequences exercise syntax, diagnostics, and document order.

