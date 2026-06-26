---
type: Skill Contract
status: active
---

# Diagnostic Protocol

`merman-lsp` is the canonical LSP transport for diagnostics, completion, and fix-backed code
actions. It projects `merman-analysis` payloads into LSP diagnostics without adding a second
analysis path, and serves both standard push diagnostics and LSP 3.17 pull diagnostics.

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
- Completion covers diagram structure, directions, operators, shapes, directives, and local
  identifiers with stable replacement edits.
- Hover, go to definition, references, prepare-rename, rename, full-document semantic tokens,
  range/delta semantic tokens, and fix-backed code actions are wired.
- `textDocument/diagnostic` and `workspace/diagnostic` are wired for pull clients; both report the
  same shared analysis payloads as the push path.
- Workspace symbols are wired from tracked document snapshots.
- Code actions remain intentionally sparse until lint rules emit source-span-backed
  `DiagnosticFix` metadata.
- Core config diagnostics include source-backed Mermaid compatibility warnings such as deprecated
  directive usage of `flowchart.htmlLabels`; diagnostics without `DiagnosticFix` metadata do not
  produce quickfixes.
- Formatting remains deferred.
