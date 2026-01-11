# ADR 0025: Mindmap Parser Technology (Hand-written, Indentation-driven)

- Status: Accepted
- Date: 2026-01-11
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid mindmap is primarily an indentation-driven, line-oriented language:

- Parent/child relationships are derived from the raw leading whitespace length of each statement line.
- Statements are simple and mostly local to a single line:
  - a node (optionally with a shaped label)
  - a decoration that mutates the most recently created node (`::icon(...)`, `:::class ...`)
  - comments / empty lines

The upstream parser (Jison) is effectively a small state machine that calls `addNode(indentLen, ...)`
and `decorateNode(...)` during parsing. For `merman`, upstream behavior is the specification: any
deviation is a bug.

## Decision

Implement mindmap parsing in `merman-core` as a hand-written, indentation-driven parser:

- Parse input line-by-line.
- Derive `level` from the raw leading whitespace character count.
- Build the tree using the same “find nearest previous node with lower level” parent lookup as Mermaid.
- Apply `::icon(...)` / `:::...` decorations to the most recently created node.
- Keep error messages aligned with Mermaid (e.g. multiple roots).

## Rationale

### Why not a grammar-first parser (`lalrpop`, `nom`) for mindmap?

- The core “syntax” is indentation, not token nesting. A grammar-first approach typically requires an
  extra INDENT/DEDENT layer or preprocessor, which adds complexity without improving parity.
- Mermaid’s mindmap semantics are DB-driven and stateful (decorate-last-node), which naturally maps
  to a small interpreter, not a pure AST pipeline.
- Grammar tooling often yields generic “expected token” errors; Mermaid’s observable behavior often
  requires domain-specific errors (e.g. root/parent rules).

### Benefits of hand-written parsing

- Tight 1:1 control over edge cases that matter for parity (base indent handling, comment stripping,
  decoration ordering).
- Lower implementation overhead for incremental parity work driven by upstream tests.
- Easier to keep behavior stable across refactors because the parsing logic is explicit and local.

## Consequences

- Mindmap parsing logic will be a dedicated module with explicit line handling; it will not share the
  same lexer/parser infrastructure as other diagram types.
- Some correctness depends on careful replication of Mermaid’s line/indent semantics; we mitigate this
  by porting upstream test vectors and adding alignment docs.

## Alternatives Considered

### Use `lalrpop` (+ an indentation preprocessor)

Pros:
- Consistent parser technology across diagrams.

Cons:
- Requires adding INDENT/DEDENT generation and integrating it into tokenization.
- Harder to keep decoration semantics and error messages aligned.

### Use `nom` combinators

Pros:
- Flexible and lightweight.

Cons:
- Still ends up implementing a manual indentation stack + state machine; the combinator layer does
  not materially reduce complexity for this language shape.

## Revisit Criteria (When to switch)

Reconsider a lexer/grammar-based approach if Mermaid mindmap evolves to include:

- Significant multi-line constructs that require real token streams (beyond indentation + single-line statements).
- Complex escaping rules that demand precise token-level spans and diagnostics.
- A formal grammar that diverges from the current DB-driven parsing model.

