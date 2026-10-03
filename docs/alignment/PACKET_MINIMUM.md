# Packet Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Packet parsing in `merman`.

Baseline: Mermaid `@11.12.3`.

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

## Mermaid 12.1 display order

Source: Mermaid `12.1.0`, commit `21f72f07ea22c0af48a3149c550654e80d8e40cb`,
`packages/mermaid/src/diagrams/packet/renderer.ts` and
`packages/mermaid/src/schemas/config.schema.yaml`.

- `packet.bitOrder` defaults to `ascending`; `descending` mirrors each row independently.
- Fields remain declared from their lowest bit to their highest bit. Semantic `start`, `end`,
  `bits`, label, row splitting, and declaration order do not change.
- A descending block's left number is its inclusive `end`; its right number is its `start`.
  Single-bit fields keep one centered number. Numbers remain absolute across rows.
- A partially filled descending row keeps its low bits at the right and unused columns at the
  left. `showBits: false` hides numbers without disabling the mirrored field positions.
- The semantic/ASCII range listing remains in declaration order; bit order is a drawing policy.

The existing Packet feature owns this configuration and rendering behavior. No new Cargo feature,
parser rule, public typed-model field, or dependency is required. The generated configuration
schema supplies editor and LSP discovery of `bitOrder` and its two values.
