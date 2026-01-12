# ADR 0037: Sankey Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `sankey` diagram is implemented using a Jison CSV parser:

- Grammar: `packages/mermaid/src/diagrams/sankey/parser/sankey.jison`
  - header: `sankey` | `sankey-beta` (case-insensitive)
  - CSV records: `source,target,value`
  - RFC4180-inspired quoting:
    - quoted fields allow commas and newlines
    - `""` represents an escaped `"`
- Pre-parsing normalization:
  - `prepareTextForParsing` collapses repeated newline runs and trims the full text
- DB behavior:
  - `packages/mermaid/src/diagrams/sankey/sankeyDB.ts`
  - node ids are sanitized and deduplicated while preserving insertion order

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `sankey` parsing in `merman-core` as a handwritten CSV parser plus a DB-like state:

- Accept `sankey` and `sankey-beta` headers (case-insensitive).
- Apply Mermaid’s `prepareTextForParsing` normalization (newline collapsing + full trim).
- Parse CSV records into:
  - nodes (in first-seen order)
  - links `{ source, target, value }`
- Apply Mermaid DB semantics:
  - per-field normalization uses `trim()` and `"" -> "` replacement
  - ids are sanitized via `sanitizeText` and stored in insertion order

## Rationale

- The diagram is fully defined by a compact CSV grammar; a dedicated Rust parser is simpler than
  introducing a Jison toolchain.
- Upstream tests for `sankey` are smoke-style; we add headless assertions in Rust to lock the
  semantic output shape.

## Consequences

- Headless output focuses on the DB-observable graph; rendering parity (layout/SVG) is out of scope
  for this phase.
- JSON cannot represent NaN/Infinity; non-finite `value` tokens are represented as `null`.

## Revisit criteria

If Mermaid expands `sankey` beyond CSV parsing or if precise token/lexer parity is required,
revisit adopting a shared lexer + grammar pipeline for CSV-like diagrams.

