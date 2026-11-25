# ADR-0013: Flowchart and Swimlane Parser Technology

## Status

Accepted

## Context

Mermaid flowchart grammar (and several other Mermaid diagrams) historically uses a Jison-based
pipeline: a lexer with multiple modes/states plus an LR-style grammar with semantic actions. This
combination is important for 1:1 compatibility because:

- the grammar is large and has ambiguous-looking constructs that depend on tokenization,
- lexer state/mode switching is used to parse labels and special segments (e.g. `|edge label|`),
- error surfaces and recovery behavior must be predictable and testable.

## Decision

- Use the shared stateful lexer for token spans and the handwritten parser in
  `crates/merman-core/src/diagrams/flowchart/parser.rs` for Flowchart and Swimlane.
- Keep open subgraphs on an explicit frame stack and append completed statements to the arena in
  walk order. Apply the edge budget before expanding each accepted edge group.
- Preserve the strict diagnostic alongside the recovery anchor used by the editor token trace.

## Consequences

- The lexer owns tokenization and exact source spans. The parser owns syntax boundaries, resource
  admission, and statement order.
- Existing semantic snapshots, editor facts, and deep-lifecycle tests cover the shared grammar.
