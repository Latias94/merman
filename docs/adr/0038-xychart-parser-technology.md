# ADR 0038: XYChart Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `xychart` diagram is implemented using a Jison grammar and a stateful DB:

- Grammar: `packages/mermaid/src/diagrams/xychart/parser/xychart.jison`
  - header: `xychart` | `xychart-beta` (case-insensitive)
  - optional chart orientation immediately after header: `vertical` | `horizontal`
  - statements:
    - `title <text>`
    - `accTitle: ...`, `accDescr: ...`, `accDescr{...}`
    - `x-axis <title?> <band|range?>`
    - `y-axis <title?> <range?>`
    - plots: `line <title?> [numbers...]`, `bar <title?> [numbers...]`
  - statement separators: newline or `;`
- DB/state behavior: `packages/mermaid/src/diagrams/xychart/xychartDb.ts`
  - axis titles and categories are sanitized (`sanitizeText`) and trimmed
  - if axes are not explicitly set, plot insertion auto-derives axis ranges
  - plot values are transformed into `[category,value]` pairs based on X axis type

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `xychart` parsing in `merman-core` as a handwritten parser plus DB-like state:

- Parse header + optional orientation (only allowed immediately after header).
- Parse statements with Mermaid-like separators (newline / `;`) while respecting bracket/brace nesting.
- Implement Mermaid’s derived semantics:
  - X axis can be band (`[categories...]`) or linear (`min --> max`)
  - Y axis is linear only (`min --> max`)
  - plot data lists must be non-empty and contain valid numbers
  - when axes are not explicitly configured, plot insertion auto-derives X/Y ranges
  - plot `values` are transformed into category pairs using Mermaid’s algorithm

## Rationale

- The grammar is compact and mostly statement-oriented; a dedicated Rust parser is faster to
  iterate for parity than introducing Jison/Langium toolchains.
- Parity is locked by porting upstream parser behavior tests (`xychart.jison.spec.ts`) into Rust
  unit tests (focusing on acceptance/rejection and semantic state).

## Consequences

- JSON cannot represent `Infinity/-Infinity`; unset axis bounds are represented as `null` in the
  headless model until concrete bounds are derived.
- Rendering (SVG, sizing, theme palette) is out of scope for this phase; this ADR is limited to
  headless parsing + semantic state.

## Revisit criteria

If XYChart grammar grows substantially or requires exact token-stream parity with Jison, revisit
implementing it via a shared lexer + grammar pipeline.

