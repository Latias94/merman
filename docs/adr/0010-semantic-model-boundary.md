# ADR-0010: Semantic Model Boundary (AST vs Family Semantics)

## Status

Accepted

## Updated

2026-07-15 for Mermaid `@11.16.0`

## Context

Mermaid diagrams are not only parsed syntactically. Their parsers construct diagram-specific
state, often called a DB, whose mutation order, validation, identifiers, and defaults are observable
by layout and rendering. Some Mermaid families are AST-first, while mature Jison-derived families
populate DB state during parsing.

Merman needs several outputs from the same meaning:

- compatibility semantic JSON for direct integrations and bindings;
- source-backed editor facts for analysis and language tooling;
- typed semantic models for layout and SVG; and
- diagnostics and metadata for validation and orchestration.

Treating compatibility JSON, editor parsing, and typed render parsing as independent masters caused
grammar and ordering behavior to drift between outputs.

## Decision

- Each built-in logical diagram family owns one successful semantic construction. The construction
  may be an AST plus a family DB, a DB populated during parsing, or another family-local typed
  representation, but it is the sole owner of grammar meaning and parse-time state.
- Compatibility JSON, typed render semantics, and editor facts are projections of that family
  construction. A recoverable editor path may accept incomplete input, but it must share the
  family's tokens and grammar facts rather than define a second successful grammar.
- The built-in Diagram Family catalog declares which projections exist for each id and alias.
  Detector, parser, editor, render, metadata, profile, and authoring-header registries derive from
  this catalog instead of repeating family facts.
- Grammar ASTs remain internal implementation details. They may support debugging, diagnostics,
  recovery, and projection, but they are not the cross-family public integration surface.
- The parse pipeline owns cross-family orchestration such as preprocessing, source remapping,
  detection, configuration, sanitization, timing, and lenient error handling. Family modules own
  semantic construction and validation.
- Rendering consumes a typed family semantic projection. Compatibility JSON is supported as an
  output contract, not as a second master render input for built-in families.
- Parity-critical parse order must remain explicit. When upstream DB behavior depends on call order,
  the family model may preserve ordering traces such as Flowchart vertex calls so downstream output
  can reproduce deterministic ids and DOM order.
- Custom semantic parser overlays may return named compatibility JSON models. They do not inherit a
  built-in typed renderer, editor parser, or family capability implicitly.

## Consequences

- Parser technology remains family-local without forcing consumers to understand every grammar
  stack.
- Semantic, editor, layout, and SVG changes converge at one family owner instead of being copied
  across registries and dispatch trees.
- Compatibility JSON stays useful and testable, but projection tests are distinct from canonical
  typed render tests.
- Typed rendering can pair a family's semantic model and layout in one opaque artifact, making
  cross-family combinations unrepresentable.
- Source-backed spans and recovery behavior become semantic projection requirements rather than an
  LSP-specific parser concern.

## Related Decisions

- ADR-0014: Upstream Parity Policy
- ADR-0071: Editor-Facing Parser and Semantic Seam
- ADR-0073: Family-Owned Diagram Architecture
