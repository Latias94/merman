# ADR-0013: Flowchart Parser Technology (Lexer + LALR)

## Status

Accepted

## Context

Mermaid flowchart grammar (and several other Mermaid diagrams) historically uses a Jison-based
pipeline: a lexer with multiple modes/states plus an LR-style grammar with semantic actions. This
combination is important for 1:1 compatibility because:

- the grammar is large and has ambiguous-looking constructs that depend on tokenization,
- lexer state/mode switching is used to parse labels and special segments (e.g. `|edge label|`),
- error surfaces and recovery behavior must be predictable and testable.

Rust parser-combinator libraries (e.g. `nom`) are excellent for many formats, but they tend to
encourage PEG/recursive-descent style parsing. For Mermaid-like grammars this can diverge from
LR/Jison behavior and make exact compatibility harder, especially once the grammar grows.

## Decision

- Use a deterministic, stateful lexer (hand-written) to produce a token stream with locations.
- Use an LR parser generator (`lalrpop`) for the flowchart grammar.
- Keep the lexer/token/span/error model shared and reusable for other large diagrams.
- Treat `nom`/PEG-style parsing as a fallback option for small, self-contained diagrams only (not
  for flowchart).

## Current implementation

- Build step runs LALRPOP: `crates/merman-core/build.rs`
- Flowchart grammar: `crates/merman-core/src/diagrams/flowchart_grammar.lalrpop`
- Flowchart lexer + adapter: `crates/merman-core/src/diagrams/flowchart.rs`

## Consequences

- Adding new flowchart syntax should primarily be “add tokens + extend grammar + add tests”.
- The lexer becomes a critical component; it must be carefully tested because tokenization affects
  parse results.
- This approach scales better to full Mermaid compatibility than ad-hoc string parsing.

