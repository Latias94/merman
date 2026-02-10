# ADR 0061: ZenUML Support (Headless Compatibility Mode)

- Status: accepted
- Date: 2026-02-10

## Context

Upstream Mermaid supports some diagram types as “external diagrams” (e.g. ZenUML). These are not
implemented in the core Mermaid parser/render pipeline and may have distinct syntax, semantics, and
rendering behavior.

`merman` treats upstream Mermaid as the spec and aims for parity. For ZenUML, full parity would
require implementing ZenUML’s behavior as observed in Mermaid `@11.12.2` when the external diagram
is registered. Upstream Mermaid’s ZenUML integration renders via `@zenuml/core` inside a browser
`<foreignObject>`, which is not available in headless pure-Rust contexts.

## Decision

- In `merman@0.1.x`, support ZenUML in a headless compatibility mode:
  - Detect `zenuml` diagrams.
  - Translate a small subset of ZenUML statements into Mermaid `sequenceDiagram` syntax.
  - Reuse the existing sequence semantic model, layout, and SVG renderer for headless output.
- Track this as “supported, not parity-gated”: we do not maintain upstream SVG baselines for ZenUML
  in the current corpus.

## Alternatives

1. Do not support external diagrams in 0.x.
   - Pros: simpler.
   - Cons: incomplete parity vs upstream Mermaid feature surface.

2. Reinterpret ZenUML as Sequence diagrams.
   - Pros: quick.
   - Cons: not 1:1 parity; diverges from upstream ZenUML’s `@zenuml/core` rendering.

3. Embed a JS engine.
   - Pros: can reuse upstream code.
   - Cons: not aligned with the “pure Rust, headless” direction.

## Consequences

- Provides a practical headless ZenUML path for basic use cases (documentation and previews).
- Leaves room for a future full implementation (e.g. a Rust port of ZenUML’s semantics/rendering)
  behind an explicit feature flag.
