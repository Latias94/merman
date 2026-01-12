# ADR 0031: Block Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s block diagram implementation consists of:

- A Jison grammar (`packages/mermaid/src/diagrams/block/parser/block.jison`) featuring multiple lexer
  modes:
  - `NODE` (shape delimiters + quoted labels)
  - `BLOCK_ARROW` / `ARROW_DIR` (blockArrow label + direction list)
  - `LLABEL` (edge label parsing between a start marker and a full link marker)
  - `CLASSDEF` / `CLASS` / `STYLE_*` (style/class statements)
- A DB layer (`packages/mermaid/src/diagrams/block/blockDB.ts`) that:
  - applies label sanitization at DB-time
  - expands `space:<n>` into `n` concrete space blocks
  - de-duplicates nodes and merges metadata
  - normalizes edge ids and aggregates edges into a list
  - records warnings when `widthInColumns` exceeds configured `columns`

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement block parsing in `merman-core` as a hand-written, cursor-based parser that mirrors the
observable behavior of Mermaid’s Jison + DB pair:

- A state-aware scanner that reproduces Jison’s token boundaries and the key lexer-mode behaviors
  needed for parity (node shapes, blockArrow, edge labels, and style statements).
- A small recursive descent parser for statements and composite block nesting (`block ... end` and
  `block:<id> ... end`).
- A DB-like post-pass that matches Mermaid’s `blockDB.ts` behavior for:
  - sanitization
  - space expansion
  - edge id normalization
  - class/style application
  - warnings

## Rationale

- The grammar is relatively small, but parity depends on lexer-mode interactions; a dedicated parser
  keeps this logic local and explicit.
- Iterating against upstream tests (`block.spec.ts`) is straightforward with a handwritten approach.
- Avoids adding a second heavy parser stack (beyond the repo-wide default strategy) until the syntax
  complexity warrants it.

## Consequences

- Parity is maintained primarily via direct test ports from Mermaid.
- Some invalid inputs may remain accepted/rejected differently until a broader set of upstream
  block-diagram tests are available; parity should be enforced by expanding the test suite as new
  upstream behaviors are identified.

## Revisit criteria

Reconsider migrating block parsing to a shared lexer + LALRPOP pipeline if:

- Mermaid expands the block grammar significantly, or
- we need stronger error reporting and span accuracy for end-user diagnostics.

