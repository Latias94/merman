# ADR-0022: Parsing Library Selection (Lexer + LALRPOP)

## Status

Accepted

## Context

`merman` is a 1:1 clone of `mermaid@11.12.2`. Many Mermaid diagrams are implemented with Jison,
using:

- a stateful lexer (multiple lexer modes/states), and
- an LALR-style grammar with precedence rules and error surfaces users rely on.

We also want `merman-core` to be headless and pure Rust, with a consistent approach that can scale
to the full Mermaid grammar set without repeated rewrites.

The team asked whether we should use a parser combinator library like `nom`, or whether a "pure
logic" parser without explicit lexing is sufficient.

## Decision

- Use a **stateful handwritten lexer** + **LALRPOP** as the default approach for Mermaid-like
  diagram grammars (flowchart/sequence/state/class/er).
- Treat "pure logic parsing without lexing" as an implementation detail at best: if we need lexer
  modes, token boundaries, and span tracking for parity, we implement them explicitly as a lexer
  rather than implicitly across ad-hoc parsing code.
- Allow localized use of other techniques only when they do not affect user-visible behavior and
  do not create a second public parsing stack (e.g. small `nom` helpers for tiny sub-parsers).

## Rationale

- **Parity with Jison**: Mermaid behavior is heavily influenced by lexer state transitions (e.g.
  class bodies, strings, generics, accessibility blocks). A stateful lexer is the closest match.
- **Maintainability**: LALRPOP keeps large grammars readable and evolvable while keeping token-level
  behavior in the lexer where it belongs.
- **Error surfaces**: Matching Mermaid errors requires token/offset/span bookkeeping; explicit
  tokenization is the most controllable foundation.

## Alternatives considered

- `nom` (parser combinators):
  - Pros: ergonomic for small parsers; good performance; easy to write incremental parsers.
  - Cons: complex precedence/ambiguity handling and lexer-mode parity tends to devolve into a
    "hand-rolled lexer inside `nom`", making it harder to match Jison behavior consistently.
- `pest` / other PEG parsers:
  - Pros: direct grammar authoring; good developer experience.
  - Cons: PEG ordered-choice semantics differ from Jison/LALR; subtle behavior drift risks.
- Fully handwritten parsers:
  - Pros: maximal control.
  - Cons: large maintenance burden across many diagrams; harder to keep consistency.

## Consequences

- Diagram parsers converge on one main technique (stateful lexer + LALRPOP), reducing long-term
  drift across diagrams.
- Some upfront work is required for tokenization and lexer states, but it pays off by keeping the
  grammar maintainable and parity-driven.

## References

- Global parser strategy: `docs/adr/0002-parser-strategy.md`
- Example diagram tech ADRs:
  - `docs/adr/0013-flowchart-parser-technology.md`
  - `docs/adr/0015-sequence-parser-technology.md`
  - `docs/adr/0016-class-parser-technology.md`
  - `docs/adr/0018-state-parser-technology.md`
