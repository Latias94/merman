# ADR-0024: DOMPurify Default Allowlists and Generation

## Status

Accepted

## Context

Mermaid uses DOMPurify to sanitize diagram labels/tooltips when `flowchart.htmlLabels` is enabled.
DOMPurify behavior is not just “remove `<script>`”, it also:

- removes unknown tags (while keeping their text content),
- removes unknown attributes,
- applies a URL attribute validation policy,
- preserves `<a target="...">` via Mermaid's DOMPurify hooks.

Our initial headless sanitizer was intentionally minimal, which caused observable drift from Mermaid
in cases like:

- unknown attributes (e.g. `<b foo="bar">`),
- custom elements (e.g. `<custom-tag>`),
- entity-decoded URL schemes in attributes (e.g. `javascript&colon;...`).

For `merman` parity, we need the same default allowlists that DOMPurify ships with (for HTML/SVG/MathML),
and a reproducible way to keep them pinned to Mermaid's baseline dependency version.

## Decision

- Pin the shared browser, reference-CLI, and Rust generation baseline to the release-contract
  version (`dompurify@3.4.17` for the Mermaid 12.2.0 reference graph). This security patch retains
  Mermaid 12.2.0, parser 2.0.1, and reference CLI 12.0.0. The accompanying Puppeteer security
  update is documented in the selected-reference receipt and security verification.
- For a dependency-only sanitizer patch, regenerate every upstream family once and inspect every
  changed SVG before accepting baseline churn. The historical 3.4.13 refresh changed no generated
  allowlist entries; its candidate SVG deltas were limited to generated node IDs, the wall-clock
  Gantt `today` marker, and external-image loading. That comparison used Mermaid's published
  bundle, which embeds its own sanitizer, and does not prove that the selected npm override ran.
- Build reference execution from Mermaid's module entry point and the selected dependency graph.
  The shared `tools/mermaid-cli/reference-runtime.mjs` builder exposes the same DOMPurify instance
  imported by Mermaid. The renderer must observe a strict HTML-label render calling that instance
  before recording its version and the built artifact's digest in baseline provenance.
- The Mermaid 12 refresh adds the upstream SVG attributes `pointer-events` and `vector-effect`
  from DOMPurify 3.4.15. Browser DOM-node and in-place mutation fixes do not map to the Rust
  string-input sanitizer API.
- DOMPurify 3.4.16 addresses GHSA-p98j-92pf-mc4p in live DOM-node sanitization with
  in-place hooks. Mermaid and the Rust sanitizer use string inputs; no corresponding live-node
  API is introduced. The published browser bundles are tested because this patch also changes
  the upstream bundler from Rollup to Rolldown.
- DOMPurify 3.4.17 addresses GHSA-cg2g-xh64-v4q7 (shadow-root template expressions),
  GHSA-mrv2-3pqr-j5jr (detached subtrees during in-place hooks), and GHSA-c6gr-qvv4-5v3f
  (SMIL `attributeName=href` in XHTML mode). The generated default sets are unchanged. Validate
  the selected reference-runtime instance and both published browser builds against the pinned
  upstream browser suite; do not infer live-DOM API parity from the Rust string-input sanitizer.
  See the [2026-10-24 security verification](../reviews/2026-10-24-dompurify-security.md) for
  signatures, real browser results, baseline generation, and remaining owner gates.
- Generate a Rust module containing DOMPurify's default allowlists via `xtask`:
  - source: `repo-ref/dompurify/dist/purify.cjs.js`
  - output: `crates/merman-core/src/generated/dompurify_defaults.rs`
- Treat `repo-ref/dompurify` as required reference material for the DOMPurify generated-artifact
  gate. If it is absent, `xtask` reports an actionable missing-reference error that points to
  `tools/upstreams/REPOS.lock.json`.
- Verify generated allowlists using:
  - DOMPurify allowlists only: `cargo run -p xtask -- verify-dompurify-defaults`
  - umbrella generated-artifact check: `cargo run -p xtask -- verify-generated`
- Implement a DOMPurify-inspired tag/attribute validation step in `merman-core::sanitize` driven by:
  - `DEFAULT_ALLOWED_TAGS` / `DEFAULT_ALLOWED_ATTR`
  - `DEFAULT_URI_SAFE_ATTRIBUTES` / `DEFAULT_DATA_URI_TAGS`
  - `DEFAULT_FORBID_CONTENTS`, SVG/MathML namespace tag sets, and HTML integration points
  - Mermaid's target-preservation hook semantics (`<a target="...">` survives sanitization, and
    `target=_blank` forces `rel=noopener`)
- Run policy validation on the recovered HTML5 tree (ADR-0021), whose attribute values already
  have browser entity-decoding semantics. Keep the separate entity decoder only for consumers
  validating serialized SVG URI attributes.

## Consequences

- `sanitizeText` / `removeScript` outputs are closer to Mermaid's actual DOMPurify behavior and more
  robust against bypass inputs.
- We can update allowlists deterministically by updating `tools/upstreams/REPOS.lock.json`,
  materializing `repo-ref/dompurify`, and regenerating.
- Default namespace transitions are covered by the generated sets and HTML5 tree builder.
  Custom element handling and the complete DOMPurify configuration surface remain incremental.

## References

- Mermaid common sanitizer: `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- DOMPurify dist baseline: `repo-ref/dompurify/dist/purify.cjs.js`
- DOMPurify source lock: `tools/upstreams/REPOS.lock.json`
- Generator: `crates/xtask/src/main.rs`
- Generated allowlists: `crates/merman-core/src/generated/dompurify_defaults.rs`
