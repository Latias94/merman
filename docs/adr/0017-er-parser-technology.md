# ADR-0017: ER Parser Technology

## Status

Accepted

## Context

Mermaid `erDiagram` (Mermaid `er` type) is defined by a Jison lexer+parser in the upstream baseline
(`repo-ref/mermaid/packages/mermaid/src/diagrams/er/parser/erDiagram.jison`).

We need:

- Deterministic, testable parsing behavior aligned to Mermaid `@11.12.2`.
- A headless semantic model suitable for rendering in other crates (SVG, CLI, GUI integrations).
- A maintainable approach that can be iterated diagram-by-diagram.

## Decision

- Implement `erDiagram` parsing in `merman-core` using:
  - A small stateful handwritten lexer in Rust (mirroring Jison state transitions).
  - A LALRPOP grammar that produces a sequence of semantic actions.
  - An in-memory DB (`ErDb`) that applies actions and produces the output model.

## Consequences

- Pros:
  - Matches Mermaid's lexer state concepts without pulling in a full JS toolchain.
  - Keeps grammar readable and change-localized to a single diagram module.
  - Enables incremental parity tests against upstream examples/specs.
- Cons:
  - Some lexical corner-cases may require iterative refinement to fully match Jison's longest-match behavior.
  - Error reporting will need future work to match Mermaid diagnostics more closely.

