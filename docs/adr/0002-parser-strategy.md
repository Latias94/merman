# ADR-0002: Parser Strategy

## Status

Accepted

## Context

Upstream Mermaid currently uses two parser stacks:

- Jison-based parsers for many mature diagrams (lexer states + LALR-style grammar).
- Langium-based parsers (Chevrotain) for some newer diagrams, producing an AST that is then mapped
  into diagram-specific "DB" structures.

`merman` needs to preserve Mermaid's user-facing behavior, including edge cases in preprocessing,
type detection, and parse error surfaces.

## Options

### Option A: Single Rust parsing stack for all diagrams

Use one parsing approach (e.g. a shared lexer + LALR parser generator) across all diagrams.

Pros:
- Uniform tooling, fewer moving parts.
- Easier to standardize error reporting and span tracking.

Cons:
- Requires rewriting some grammars (PEG vs LALR differences, lexer modes, etc.).
- Higher upfront cost for large grammars (flowchart, sequence, state, class...).

### Option B: Diagram-local parsing implementations behind a stable interface

Define a stable public parsing interface and allow each diagram to use the most suitable internal
parser (hand-written, LALR, PEG, etc.).

Pros:
- Incremental delivery with clear boundaries.
- Can mirror upstream behavior per diagram without global constraints.

Cons:
- Potentially higher long-term maintenance cost if too many different techniques are used.

## Recommendation

Adopt Option B initially, but constrain the allowed internal parser techniques to a small set and
prefer convergence to a shared lexer/token/span/error model over time.

This keeps early milestones achievable while still aiming for long-term consistency.

## Decision

- `merman` exposes one stable public parsing interface; internal implementations may differ per
  diagram.
- Internals must share:
  - tokenization conventions where possible,
  - span/source-location tracking,
  - error surface (types + messages + recoverability policy),
  - preprocessing + detector behavior (these are part of compatibility).
- For Jison-heavy, lexer-mode-heavy diagrams (e.g. flowchart/sequence/state/class), prefer a
  deterministic state-machine lexer (mode switching) and an LALR(1)-style grammar implementation,
  because it most closely matches Mermaid's historical behavior.
- For smaller or AST-first diagrams (e.g. the subset currently parsed via `@mermaid-js/parser`),
  allow simpler implementations (hand-written parser / parser combinators), but they must still
  produce the same user-visible behavior and integrate with the shared token/span/error model.
- Do not expose “two public parser stacks” (e.g. “Jison-style API vs Langium-style API”) at the
  crate boundary. The difference is an internal detail.
