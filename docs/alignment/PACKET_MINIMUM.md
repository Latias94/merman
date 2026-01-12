# Packet Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Packet parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Parser: `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/parser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/db.ts`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/packet/packet.spec.ts`

## Supported (current)

- Header:
  - `packet` and `packet-beta`.
  - Allows empty lines above the header (preprocessing trims leading whitespace).
- Common metadata:
  - `title ...`
  - `accTitle: ...`
  - `accDescr: ...` and `accDescr{...}`
  - Last assignment wins.
- Blocks:
  - Explicit ranges: `start-end: "label"` (inclusive `start` / `end`).
  - Single bits: `start: "label"` (same as `start-start`).
  - Relative bit counts: `+bits: "label"` where `start` is inferred from the previous block.
  - Labels are quoted strings (`"..."` or `'...'`) with backslash escapes.
- Validation / DB behavior:
  - Blocks must be contiguous; otherwise error:
    - `Packet block <start> - <end> is not contiguous. It should start from <expected>.`
  - Explicit `end < start` is rejected:
    - `Packet block <start> - <end> is invalid. End must be greater than start.`
  - `+0` is rejected:
    - `Packet block <start> is invalid. Cannot have a zero bit field.`
- Row splitting:
  - Blocks are split across rows using Mermaid’s `getNextFittingBlock` logic.
  - Row width is `packet.bitsPerRow` (default `32`).

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s Packet DB behavior:
  - `type`
  - `title`, `accTitle`, `accDescr`
  - `packet`: an array of words (rows); each word is an array of blocks:
    - `{ start, end, bits, label }`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `packet` grammar and DB behavior
compatibility at the pinned baseline tag.
