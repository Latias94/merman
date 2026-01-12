# ADR 0035: Radar Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `radar` diagram is currently exposed as `radar-beta` and implemented using the
`@mermaid-js/parser` Langium grammar:

- Grammar: `packages/parser/src/language/radar/radar.langium`
  - header variants: `radar-beta`, `radar-beta:`, `radar-beta :`
  - statements:
    - `axis A,B,C` where each axis may have a label: `A["Axis A"]`
    - `curve curveName{1,2,3}` with optional label: `curveName["Label"]{...}`
      - entries can be numeric (`1,2,3`) or detailed (`A: 1, B: 2, C: 3`)
    - options: `showLegend <bool>`, `ticks <number>`, `min <number>`, `max <number>`,
      `graticule <circle|polygon>`
- Mermaid DB behavior:
  - `packages/mermaid/src/diagrams/radar/db.ts`
  - ordering of detailed curve entries is derived from the axis list

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `radar` parsing in `merman-core` as a handwritten parser that produces a headless model
aligned with Mermaid’s DB-observable state:

- Accept `radar-beta` header variants.
- Parse:
  - `title`, `accTitle`, `accDescr`
  - `axis` lists with optional quoted labels
  - `curve` definitions (supports both numeric entries and detailed `Axis: value` entries)
  - option statements with Mermaid defaults and “last value wins” semantics
- Apply Mermaid DB semantics:
  - axis label defaults to axis name
  - curve label defaults to curve name
  - detailed entries are reordered to match the axis list
  - missing detailed entry throws `Missing entry for axis <axis label>`

## Rationale

- The diagram grammar is small and largely statement-oriented; a custom parser is straightforward.
- Parity is maintained by porting upstream tests (`radar.spec.ts`) into Rust unit tests.

## Consequences

- The implementation must preserve Mermaid’s derived behavior (notably reordering detailed entries
  based on axes and the exact error message for missing entries).
- Rendering parity (SVG output) is explicitly out of scope for this phase; this ADR is about
  headless parsing and semantic state only.

## Revisit criteria

If the grammar grows substantially or if we need full parser error spans compatible with Mermaid’s
Langium parser, revisit implementing the `radar` grammar via the shared lexer + grammar toolchain.

