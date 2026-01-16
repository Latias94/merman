# ADR-0010: Semantic Model Boundary (AST vs DB-like Model)

## Status

Accepted

## Context

Mermaid diagrams are not only parsed syntactically; they are transformed into diagram-specific data
structures ("DB" objects) that renderers and layout engines consume. Some Mermaid diagrams are
AST-first (Langium-based), while many mature diagrams directly populate DB structures during parse
(Jison-based).

`merman` needs a headless output that is stable and useful for:

- direct integrations (UI frameworks),
- CLI validation,
- rendering crates (SVG output),
- future layout engine implementations.

## Decision

- `merman-core` exposes a parse API that returns:
  - detected diagram type,
  - merged config overrides (compatibility shape),
  - effective config,
  - front-matter title (if any).
- Rendering crates must consume a stable semantic model (DB-like model) rather than raw grammar AST.
- Grammar AST is considered an internal implementation detail:
  - It may exist for debugging and diagnostics.
  - It is not the primary integration surface.
- Exception for parity-critical ordering: when upstream DB objects depend on parse-time call order (e.g. Flowchart FlowDB `vertexCounter`),
  the semantic model may include explicit ordering traces (e.g. `vertexCalls`) so renderers can reproduce upstream DOM ids deterministically.

## Consequences

- Downstream renderers can evolve independently from grammar choices.
- We avoid forcing consumers to understand grammar details or multiple parsing stacks.
