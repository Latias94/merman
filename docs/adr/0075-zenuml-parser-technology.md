# ADR 0075: ZenUML Parser Technology

- Status: accepted
- Date: 2026-07-20
- Baseline: Mermaid `11.16.0`, selected ZenUML Core `3.50.1`

## Context

ZenUML is defined by the selected ZenUML Core lexer/parser grammar rather than Mermaid's Sequence
grammar. The upstream ANTLR grammar is not a context-free rule list that can be transferred to an
LR table without retaining custom behavior. It uses lexer channels and modes, runtime semantic
predicates for title, divider, number, and name boundaries, Unicode-aware lookahead, and explicit
tolerance for incomplete blocks. Those behaviors are observable in editor recovery and source
ranges as well as strict parsing.

The implementation under `crates/merman-core/src/diagrams/zenuml/` already has two deliberate
layers:

1. a Unicode token scanner derived from `sequenceLexer.g4`, including the selected channels,
   modes, predicates, comments, closed/in-progress strings, and exact byte spans;
2. a bounded recursive-descent grammar derived from `sequenceParser.g4`, including local recovery,
   expression boundaries, same-line statements, nested fragments, and exact subexpression spans.

The parser constructs one AST/semantic result used by strict parsing, recovered editor facts,
analysis, LSP, typed rendering, and compatibility projections. Focused tests prove same-line rule
boundaries, parameter and condition spans, recovery after an invalid token without line
synchronization, bounded nesting, and the selected upstream corpus.

The U4 implementation plan originally required translating ZenUML into an existing Rust parser
generator. That wording confused a possible implementation tool with the actual architecture and
behavior contract. A generated LR parser would still need a custom lexer, semantic-predicate
adaptation, recovery design, span propagation, and the family semantic projection. Rewriting the
working grammar port solely to satisfy tool uniformity would add a second migration without
evidence that it improves correctness or maintainability.

## Decision

ZenUML uses its grammar-derived Unicode token scanner and bounded recursive-descent parser.
Parser-generator uniformity is not an architecture invariant.

The implementation must preserve these invariants:

- The selected ZenUML Core lexer/parser sources and executable corpus are the behavior authority.
- Token kinds, rule boundaries, precedence, semantic predicates, channels/modes, and recovery are
  ported explicitly; regex and line-oriented parsing are prohibited for nested syntax.
- Every syntax and semantic fact carries an original-source byte span or an explicit recovered
  insertion span.
- Strict and recovered entry points share the same tokens and grammar. No editor-, LSP-, Web-, or
  renderer-specific ZenUML parser may exist.
- Nesting, source, label, participant, statement, fragment, and SVG limits remain explicit and
  testable.
- A selected upstream release change must update the grammar delta inventory and positive,
  malformed, semantic, editor, and render corpus evidence before capability claims change.

A parser generator may replace this implementation later only when a concrete prototype proves
that it preserves the same source-backed behavior and recovery while materially reducing owned
complexity. The migration must delete the prior parser in the same change; parallel successful
parsers are not allowed.

## Consequences

- U4 is complete without an unnecessary parser-technology rewrite.
- The non-generated parser remains substantial, but its complexity corresponds to language
  behavior that an LR front end would not remove.
- Review and release alignment focus on rule/corpus deltas and single-semantic ownership instead of
  private function names or a preferred parsing crate.
- Exact spans and local recovery remain first-class contracts for analysis, Monaco, the unpublished
  VS Code extension, and LSP refactoring.

## Rejected Alternatives

### Force ZenUML through the Flowchart LALRPOP stack

Rejected. Flowchart and ZenUML have different upstream technologies and recovery contracts. Sharing
a generator would not share a grammar or semantic model and would require adapting the ZenUML
lexer predicates and modes around the generated parser.

### Generate and retain a second parser for comparison

Rejected. Two successful parsers would split strict, editor, and render semantics. Executable
upstream corpus evidence is the comparison oracle.

### Return to regex or line parsing

Rejected. Same-line statements, nested fragments, expressions, Unicode identifiers, and local
recovery cannot be represented faithfully by that approach.

## Related Decisions

- ADR-0010: Semantic Model Boundary
- ADR-0061: Source-Backed ZenUML Support
- ADR-0071: Editor Parser Semantic Seam
- ADR-0073: Family-Owned Diagram Architecture
