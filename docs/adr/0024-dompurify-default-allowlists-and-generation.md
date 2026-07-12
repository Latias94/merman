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

- Pin DOMPurify baseline to Mermaid's resolved dependency version (`dompurify@3.4.0` for
  `mermaid@11.16.0`).
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
  - Mermaid's target-preservation hook semantics (`<a target="...">` survives sanitization, and
    `target=_blank` forces `rel=noopener`)
- Decode the minimal subset of HTML entities required for URI attribute parity (notably `&colon;`,
  `&newline;`, `&tab;`, and common numeric `:` forms), because DOMPurify runs on a parsed DOM where
  those entities are already decoded by the browser.

## Consequences

- `sanitizeText` / `removeScript` outputs are closer to Mermaid's actual DOMPurify behavior and more
  robust against bypass inputs.
- We can update allowlists deterministically by updating `tools/upstreams/REPOS.lock.json`,
  materializing `repo-ref/dompurify`, and regenerating.
- Full DOMPurify parity is still a long-term effort (namespaces, custom element handling, full config
  surface), but the foundation matches Mermaid's default behavior much better.

## References

- Mermaid common sanitizer: `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`
- DOMPurify dist baseline: `repo-ref/dompurify/dist/purify.cjs.js`
- DOMPurify source lock: `tools/upstreams/REPOS.lock.json`
- Generator: `crates/xtask/src/main.rs`
- Generated allowlists: `crates/merman-core/src/generated/dompurify_defaults.rs`
