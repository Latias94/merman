---
type: Skill Contract
status: active
---

# Diagnostic Protocol

`merman-lsp` projects `merman-analysis` payloads into LSP diagnostics without adding a second
analysis path.

## Canonical rules

- Source of truth: `merman-analysis::AnalysisPayload`
- Transport: `tower-lsp`
- Coordinate system: UTF-16 LSP positions
- Markdown fences: remapped to the host document URI and range

## Compatibility

- Plain Mermaid documents publish diagnostics against the file URI directly.
- Markdown/MDX documents publish diagnostics against the containing document URI.

## Residuals

- Client font metrics, rendering, and HTML label behavior are not part of the LSP contract.
- Completion is shallow and structural in this first pass, but now covers diagram structure, directions, shapes, and local identifiers.
