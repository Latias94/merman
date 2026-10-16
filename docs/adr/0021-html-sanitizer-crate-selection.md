# ADR-0021: HTML Sanitizer Crate Selection

## Status

Revised by the shared HTML5 fragment refactor in October 2026.

## Context

Mermaid uses DOMPurify in the browser to sanitize labels and links. For `merman-core` we want a
pure-Rust, headless implementation that:

- Is robust against malformed HTML (no regex-based HTML parsing).
- Preserves Mermaid-like output stability (minimal rewriting; keep original structure where possible).
- Supports Mermaid-specific hooks (e.g. add `rel="noopener"` when `target="_blank"`).

The user also asked whether Dioxus ecosystem crates (e.g. `blitz`) are suitable for this.

## Decision

- Use `html5ever` for HTML5 fragment tokenization and tree construction, with a private flat tree sink in `merman-core::sanitize`.
  - Recover missing end tags, misnested formatting, implicit containers, and foreign-content transitions before policy traversal.
  - Keep node ownership flat and traverse iteratively so nesting does not create recursive teardown.
  - Admit input and tree storage through the caller-owned sanitizer controls, check cancellation between bounded input chunks and during traversal, and stop after a failed callback without processing subsequent chunks or EOF.
- Apply DOMPurify-like sanitization to the recovered tree:
  - remove `<script>` / `<iframe>` / `<style>` elements
  - validate tags/attributes using DOMPurify default allowlists (HTML/SVG/MathML), generated from the
    pinned DOMPurify version
  - validate URL-like attributes using a DOMPurify-inspired policy (allowed schemes + data-URI tag exceptions)
  - apply Mermaid-specific `<a>` handling (`target` preservation + `target="_blank"` -> `rel="noopener"`)
- Do not use layout/rendering engines like `blitz` for sanitization (out of scope; heavy dependency; wrong abstraction).
- Serialize semantic HTML and SVG-embedded XHTML through distinct output modes. The XHTML serializer owns XML escaping, element closure, and namespace declarations; final SVG XML validation remains mandatory.
- Match DOMPurify's inert document initialization and body selection, including leading whitespace. Architecture's subsequent SVG `innerHTML` assignment uses a separate context-aware fragment parse without repeating the sanitization policy.
- Reuse prepared final XHTML for Flowchart measurement and emission while preserving reference-required entity and sanitization stages.

## Reason for Revision

The previous `lol_html` implementation removed unsafe source constructs but did not build and serialize a repaired HTML5 tree.
For example, an authored label `<b>API` survived the sanitizer unchanged and became `<p><b>API</p>` in Flowchart SVG, which failed strict XML validation.
Appending closing tags locally cannot reproduce formatting reconstruction, implied table containers, or foreign-content parsing.
The former hand-written HTML-to-XHTML normalizers also allowed measurement and output to interpret the same label differently.
The replacement uses the maintained parser's tree-building rules and keeps Merman's policy and output adaptation explicit.

The dependency's unoptimized tokenizer/tree-builder has substantial fixed stack frames: even a one-span input exceeds a 64 KiB Windows debug stack.
Lifecycle canaries therefore use 512 KiB in unoptimized builds and retain the 64 KiB optimized canary at 2,048 levels; this is a build-configuration constraint rather than permission for input-depth recursion.

## Alternatives considered

- `ammonia` (html5ever-based sanitizer):
  - Pros: battle-tested sanitizer
  - Cons: harder to implement Mermaid's per-element hook behavior and preserve output shape/attribute ordering
- `html5ever` + custom DOM traversal/serialization:
  - Selected: the parser owns HTML5 algorithms; a private tree sink owns resource admission and stack-safe storage; existing generated policy remains authoritative.
  - Costs: tree storage and two serialization contracts require focused namespace, resource, and parity tests.
- `lol_html` streaming rewriting:
  - Useful for low-buffer source transformations, but does not establish Merman's recovered-tree and XHTML output contracts.
- Dioxus `blitz`:
  - Pros: strong layout engine for UI
  - Cons: not a sanitizer; does not solve DOMPurify parity; too heavy for `merman-core`

## Consequences

- Sanitization logic stays pure Rust and headless-friendly.
- Parity improvements can be driven by upstream Mermaid test vectors plus additional regression tests.
- Parser and serializer differences must be explained against the pinned reference rather than hidden by comparator normalization.

## References

- DOMPurify allowlist generation: `docs/adr/0024-dompurify-default-allowlists-and-generation.md`
