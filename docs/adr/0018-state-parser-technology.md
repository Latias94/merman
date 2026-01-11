# ADR-0018: State Parser Technology

## Status

Accepted

## Context

Mermaid `stateDiagram` (including `stateDiagram-v2`) is defined by a Jison lexer+parser in the
upstream baseline (`repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/stateDiagram.jison`)
and populated into `StateDB` (`repo-ref/mermaid/packages/mermaid/src/diagrams/state/stateDb.ts`).

We need:

- Deterministic, testable parsing behavior aligned to Mermaid `@11.12.2`.
- A headless semantic model suitable for renderer integration in other crates (SVG, CLI, GUI).
- A maintainable approach consistent with other diagram parsers in `merman-core`.

## Decision

- Implement `stateDiagram` parsing in `merman-core` using:
  - A small stateful handwritten lexer in Rust (mirroring Jison start conditions where relevant).
  - A LALRPOP grammar that builds a statement list (similar to the upstream statement shapes).
  - An in-memory DB (`StateDb`) that:
    - Applies Mermaid v2 start/end rewriting (`[*]` -> `root_start`/`root_end`, nested via parent id).
    - Applies class/style statements (`classDef`, `class`, `style`) to states.
    - Produces a headless semantic model.

## Consequences

- Pros:
  - Keeps parsing fully within Rust while preserving upstream lexer concepts.
  - Enables incremental parity via spec-port tests without a JS toolchain dependency.
  - Matches the repository-wide approach used by `classDiagram` and `erDiagram`.
- Cons:
  - Some upstream features (notes, clicks, full layout data) will require iterative expansion.
  - Error reporting will need future work to match Mermaid diagnostics precisely.

