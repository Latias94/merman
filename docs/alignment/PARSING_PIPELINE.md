# Parsing Pipeline Alignment (Mermaid 11.12.x)

This document describes the user-visible parsing pipeline we need to match.

## Baseline

Upstream baseline: `mermaid@11.12.2` (see `docs/adr/0001-upstream-baseline.md`).

## High-level steps

1. Normalize text (CRLF -> LF, HTML attribute quoting normalization).
2. Extract YAML front-matter (if present and properly closed).
3. Extract directives (`%%{init: ...}%%`, `%%{initialize: ...}%%`, `%%{wrap}%%`).
4. Merge configuration (front-matter config merged with directive config, directive wins).
5. Remove comments (lines starting with `%%`, excluding directive blocks).
6. Detect diagram type using registered detectors (order matters).

## Output alignment

- The parse API returns the merged overrides extracted from front-matter and directives as
  `config` (this mirrors Mermaid's `mermaidAPI.parse()`).
- Consumers may also need the full effective config (`site defaults + overrides`) for subsequent
  parsing/rendering steps.

## Special cases

- Text beginning with `---` that is not valid YAML front-matter must produce the "malformed
  front-matter" error message (Mermaid registers a special `---` diagram type for this).

## Error message alignment

- High-frequency, user-facing errors should match Mermaid's baseline message text.
- Internal/low-frequency errors prioritize stable categories and payloads first, then message text.
