# ADR 0030: Architecture Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s architecture diagram uses the shared parser package (`@mermaid-js/parser`) rather than a
Jison grammar:

- Grammar: `packages/parser/src/language/architecture/architecture.langium`
- Common terminals for title/accessibility: `packages/parser/src/language/common/common.langium`
- Mermaid integration: `packages/mermaid/src/diagrams/architecture/architectureParser.ts` populates
  `ArchitectureDB` from the parsed AST.

`merman` must provide a headless parser where upstream behavior is the spec.

## Decision

Implement architecture parsing in `merman-core` as a hand-written, line-oriented parser that
reproduces the observable behavior of the Langium grammar and the DB validations (for the currently
implemented slice):

- Require the `architecture-beta` header (allowing `title ...` on the same line).
- Parse `title`, `accTitle`, `accDescr` statements with Mermaid-compatible comment termination.
- Parse `group`, `service`, `junction`, and colon-form `edge` statements.
- Apply DB-like validation for:
  - id collisions between nodes and groups
  - group placement rules for `in <parent>`
  - edge direction validity and “endpoint must exist” constraints

Rationale: the current architecture grammar is line-based; a hand-written parser keeps the runtime
simple, avoids embedding Langium/JS, and can be expanded incrementally as parity work progresses.

## Consequences

- Parity is maintained by porting Mermaid’s upstream tests:
  - `packages/mermaid/src/diagrams/architecture/architecture.spec.ts`
  - `packages/parser/tests/architecture.test.ts` (where applicable)
- Future renderer crates can consume the headless snapshot without a JS runtime.

## Revisit criteria

Reconsider if Mermaid significantly expands the architecture syntax or if we decide to directly
port the Langium grammar to Rust with a dedicated parser generator.

