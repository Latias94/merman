# ADR 0034: Packet Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `packet` diagram is implemented using the `@mermaid-js/parser` Langium grammar:

- Grammar: `packages/parser/src/language/packet/packet.langium`
  - header: `packet` | `packet-beta`
  - blocks: `start-end: "label"` or `+bits: "label"`
  - common metadata: `title`, `accTitle`, `accDescr`
- Mermaid DB behavior: `packages/mermaid/src/diagrams/packet/db.ts`
- Semantic population / row splitting logic:
  - `packages/mermaid/src/diagrams/packet/parser.ts`

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `packet` parsing in `merman-core` as a handwritten, line-oriented parser with
Mermaid-aligned DB behavior:

- Accept both `packet` and `packet-beta` headers (detector mirrors Mermaid).
- Parse `title`, `accTitle`, `accDescr` and packet blocks.
- Apply Mermaid’s contiguity and validation rules (including error messages).
- Split blocks across rows using Mermaid’s `getNextFittingBlock` logic and `packet.bitsPerRow`.

## Rationale

- The grammar is line-based and compact; a custom parser is straightforward and fast to iterate on.
- Parity is locked by porting upstream tests (`packet.spec.ts`) into Rust unit tests.

## Consequences

- Some Mermaid quirks are preserved intentionally (e.g. split-block `bits` values computed as
  `end - start` in `getNextFittingBlock`).
- For phase 1, the headless output focuses on the DB-observable state (`packet` words + common
  metadata).

## Revisit criteria

If `packet` expands beyond its current line-oriented grammar or if we need full-fidelity syntax
errors/spans, reconsider moving it onto the shared lexer + grammar toolchain.

