# ADR 0016: Class Diagram Parser Technology

## Status

Accepted

## Context

`merman` targets full 1:1 behavioral parity with `mermaid@11.12.3` for `classDiagram`.

Mermaid implements `classDiagram` parsing using a Jison lexer with multiple lexer states
(e.g. class body, namespace body, strings, generics, accessibility blocks) and a grammar that
drives side effects into `ClassDB`.

We need a Rust implementation that is:

- headless (no DOM assumptions)
- deterministic and testable
- incrementally extendable to full Mermaid coverage without rewrites

## Decision

Use a stateful lexer and a handwritten predictive grammar over normalized, spanned tokens. The
parser keeps an explicit stack for namespace bodies and emits actions for the headless `ClassDb`.
Renderers and converters consume the resulting classes, relations, notes, and namespaces.

## Consequences

- The lexer owns class body member lines and token spans. The grammar owns statement boundaries,
  expected tokens, and namespace replay order.
- Cancellation is checked during scanning, normalization, parsing, and DB replay.
- The editor can recover symbols from the normalized event tape after a grammar failure.

## References

- Upstream grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/class/parser/classDiagram.jison`
- Upstream DB: `repo-ref/mermaid/packages/mermaid/src/diagrams/class/classDb.ts`
- [Family compatibility notes](../alignment/FAMILY_NOTES.md#class)

