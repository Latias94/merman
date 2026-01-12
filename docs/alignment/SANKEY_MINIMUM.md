# Sankey Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Sankey parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `sankey`
  - `sankey-beta`
  - Header is matched case-insensitively (mirrors Jison `%options case-insensitive`).
- Pre-parsing normalization:
  - Collapses repeated newline runs (`\r`/`\n`) into a single `\n`.
  - Trims the full text (mirrors Mermaid `prepareTextForParsing`).
- CSV records:
  - Each record is `source,target,value`.
  - Fields:
    - unquoted fields end at `,` or newline
    - quoted fields use `"` and support escaped quotes via `""`
  - Field normalization matches Mermaid:
    - `trim()` then `"" -> "`
  - `value` is parsed as a floating-point number.
- Node/edge semantics:
  - Nodes are created in first-seen order and deduplicated by id.
  - Node ids are sanitized using the shared Mermaid sanitizer (matches `sankeyDB.ts` behavior).
  - Links preserve parse order.
- Security:
  - `__proto__` is treated as a normal id (safe in Rust; upstream explicitly tests this).

## Output shape (Phase 1)

- Headless semantic output aligned with Mermaid’s `sankeyDB.getGraph()`:
  - `type`
  - `graph.nodes`: `{ id }[]`
  - `graph.links`: `{ source, target, value }[]`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `sankey` grammar and DB behavior
compatibility at the pinned baseline tag.

