# ADR-0021: HTML Sanitizer Crate Selection

## Status

Accepted

## Context

Mermaid uses DOMPurify in the browser to sanitize labels and links. For `merman-core` we want a
pure-Rust, headless implementation that:

- Is robust against malformed HTML (no regex-based HTML parsing).
- Preserves Mermaid-like output stability (minimal rewriting; keep original structure where possible).
- Supports Mermaid-specific hooks (e.g. add `rel="noopener"` when `target="_blank"`).

The user also asked whether Dioxus ecosystem crates (e.g. `blitz`) are suitable for this.

## Decision

- Use `lol_html` (streaming HTML rewriter) as the foundation for DOMPurify-like sanitization:
  - remove `<script>` / `<iframe>` / `<style>` elements
  - validate tags/attributes using DOMPurify default allowlists (HTML/SVG/MathML), generated from the
    pinned DOMPurify version
  - validate URL-like attributes using a DOMPurify-inspired policy (allowed schemes + data-URI tag exceptions)
  - apply Mermaid-specific `<a>` handling (`target` preservation + `target="_blank"` -> `rel="noopener"`)
- Do not use layout/rendering engines like `blitz` for sanitization (out of scope; heavy dependency; wrong abstraction).

## Alternatives considered

- `ammonia` (html5ever-based sanitizer):
  - Pros: battle-tested sanitizer
  - Cons: harder to implement Mermaid's per-element hook behavior and preserve output shape/attribute ordering
- `html5ever` + custom DOM traversal/serialization:
  - Pros: maximal control
  - Cons: more code to reach Mermaid parity; easy to drift; serializer differences
- Dioxus `blitz`:
  - Pros: strong layout engine for UI
  - Cons: not a sanitizer; does not solve DOMPurify parity; too heavy for `merman-core`

## Consequences

- Sanitization logic stays pure Rust and headless-friendly.
- Parity improvements can be driven by upstream Mermaid test vectors plus additional regression tests.

## References

- DOMPurify allowlist generation: `docs/adr/0024-dompurify-default-allowlists-and-generation.md`
