# ADR 0033: QuadrantChart Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s `quadrantChart` implementation uses:

- A Jison grammar (`packages/mermaid/src/diagrams/quadrant-chart/parser/quadrant.jison`) with:
  - case-insensitive matching for keywords
  - quoted and markdown strings for text tokens
  - strict point coordinate tokenization (`1` or `0` or `0.<digits>`)
  - `classDef` / `:::className` constructs for shared point styling
- A DB layer (`packages/mermaid/src/diagrams/quadrant-chart/quadrantDb.ts`) that:
  - sanitizes text at DB-time
  - validates style keys/values (`utils.ts`) and throws user-visible errors
  - stores points in reverse insertion order via `addPoints([new], existing)`

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement `quadrantChart` parsing in `merman-core` as a handwritten, line-oriented parser plus a
DB-like state object:

- Parse the `quadrantChart` header (case-insensitive).
- Parse one statement per line (and `;` separators), mirroring the Jison grammar’s observable
  behavior:
  - `title`, `accTitle`, `accDescr` (including `{ ... }` multi-line blocks)
  - `x-axis` / `y-axis` with `--+>` delimiter semantics
  - `quadrant-[1..4]`
  - points with optional `:::className` and style lists
  - `classDef` for reusable styles
- Apply DB behavior aligned with Mermaid:
  - sanitize axis/quadrant/point text at DB-time
  - validate style keys/values and preserve error messages
  - prepend points to match Mermaid’s insertion order

## Rationale

- The diagram is effectively line-based and small; a dedicated parser is simpler than introducing
  an additional grammar toolchain.
- Parity is driven by porting upstream tests and examples; a handwritten parser makes it easy to
  patch exact behaviors as they are discovered.

## Consequences

- The implementation must keep matching Mermaid’s tokenization quirks (notably coordinate tokens and
  axis delimiter handling).
- Some Jison-only parsing details (e.g. exact token streams) are intentionally not exposed; the
  headless output focuses on the DB-observable state.

## Revisit criteria

Reconsider migrating `quadrantChart` parsing to a shared lexer + grammar pipeline if the diagram
grammar expands beyond line-oriented constructs or if error span fidelity becomes critical.

