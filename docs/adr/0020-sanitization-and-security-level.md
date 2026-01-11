# ADR-0020: Sanitization and `securityLevel`

## Status

Accepted

## Context

Mermaid uses `common.sanitizeText*` and `utils.formatUrl` to make diagram labels, tooltips, and links
safe-by-default. This behavior is driven by `securityLevel` and other config flags like
`flowchart.htmlLabels`.

For a 1:1 Mermaid clone, these transformations must happen consistently in the headless semantic
model so downstream renderers (SVG/Canvas/UI wrappers) can match Mermaid behavior.

## Decision

- Implement a Mermaid-inspired sanitizer in `merman-core`:
  - `sanitize::sanitize_text` and `sanitize::sanitize_text_or_array`
  - `sanitize::remove_script` for strict/antiscript levels
  - HTML rewriting uses `lol_html` (streaming HTML rewriter) to avoid brittle regex-based parsing and
    to keep output close to input while applying Mermaid's safety rules.
  - Tag/attribute validation uses DOMPurify default allowlists generated from the pinned DOMPurify version
    (see ADR-0024).
  - `dompurifyConfig` coverage is incremental; currently supported keys include:
    - `ALLOWED_TAGS` / `ADD_TAGS` / `FORBID_TAGS`
    - `ALLOWED_ATTR` / `ADD_ATTR` / `FORBID_ATTR`
    - `ALLOW_ARIA_ATTR` / `ALLOW_DATA_ATTR` / `ALLOW_UNKNOWN_PROTOCOLS`
    - `ADD_URI_SAFE_ATTR` / `ADD_DATA_URI_TAGS`
    - `KEEP_CONTENT`
- Implement URL formatting in `utils::format_url` (Mermaid `utils.formatUrl` parity):
  - if `securityLevel != loose`, sanitize URLs using a Rust port of `@braintree/sanitize-url@7.1.1`
    (Mermaid dependency), returning `about:blank` for unsafe schemes and normalizing some http/https URLs
- Apply sanitization/formatting at the semantic-model boundary:
  - Flowchart: node labels, edge labels, tooltips, and click link URLs
  - State: layout `nodes[].label`, `edges[].label`, and note text in layout nodes

## Consequences

- The headless model aligns better with Mermaid defaults and security behavior.
- The current sanitizer is a targeted implementation driven by upstream test cases and common
  patterns; full DOMPurify parity (and `dompurifyConfig`) remains an explicit follow-up item.
