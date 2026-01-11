# ADR 0016: Class Diagram Parser Technology

## Status

Accepted

## Context

`merman` targets full 1:1 behavioral parity with `mermaid@11.12.2` for `classDiagram`.

Mermaid implements `classDiagram` parsing using a Jison lexer with multiple lexer states
(e.g. class body, namespace body, strings, generics, accessibility blocks) and a grammar that
drives side effects into `ClassDB`.

We need a Rust implementation that is:

- headless (no DOM assumptions)
- deterministic and testable
- incrementally extendable to full Mermaid coverage without rewrites

## Decision

Implement `classDiagram` parsing using:

- a stateful handwritten lexer (Rust), and
- a LALRPOP grammar (Rust),

mirroring Mermaid's Jison lexer states and operator precedence where relevant.

The semantic output is a DB-like model (classes/relations/notes/namespaces) suitable for reuse by
other crates (renderers, CLI, converters).

## Consequences

### Positive

- Matches Mermaid's "lexer state" approach (e.g. class body member lines) closely, reducing
  impedance mismatches.
- Keeps the grammar maintainable while allowing precise token-level behavior where needed.
- Enables incremental parity: we can add statements and edge cases without destabilizing unrelated
  diagrams.

### Negative

- Requires ongoing maintenance of lexer states and tokenization details to reach full parity.
- Error reporting parity (Jison expected tokens/locations) will require extra work.

## References

- Upstream grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/class/parser/classDiagram.jison`
- Upstream DB: `repo-ref/mermaid/packages/mermaid/src/diagrams/class/classDb.ts`
- Alignment slice: `docs/alignment/CLASS_MINIMUM.md`

