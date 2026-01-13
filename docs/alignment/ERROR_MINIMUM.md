# Error Diagram Minimum Slice (Phase 1)

This document defines the minimum slice for Mermaid `error` diagram support in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header: `error` (case-insensitive, trimmed), detected by Mermaid's diagram registry.
- Parsing: headless parsing is a no-op and returns a minimal semantic model.

## Output shape (Phase 1)

- Headless semantic snapshot:
  - `type`: always `error`

## Alignment goal

Match Mermaid `error` diagram behavior at the pinned baseline tag.
