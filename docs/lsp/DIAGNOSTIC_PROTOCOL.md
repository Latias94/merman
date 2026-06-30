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

## Current Surface

- Client font metrics, rendering, and HTML label behavior are not part of the LSP contract.
- Completion covers diagram structure, directions, operators, shapes, directives, and local
  identifiers with stable replacement edits.
- Hover, go to definition, references, prepare-rename, rename, full-document semantic tokens,
  range/delta semantic tokens, and fix-backed code actions are wired.
- That claim applies to the first-class matrix in `CAPABILITIES.md`; `error` remains an internal
  fallback diagram rather than a product-family contract.
- `textDocument/diagnostic` is wired for pull clients and reports the same shared analysis payloads
  as the push path. `workspace/diagnostic` is not advertised or implemented until unopened-file
  workspace scanning exists.
- Workspace symbols are wired from tracked document snapshots.
- Core config diagnostics include source-backed Mermaid compatibility warnings such as deprecated
  directive usage of `flowchart.htmlLabels` (now with a preferred migration quickfix) and
  deprecated external diagram loading config; diagnostics without `DiagnosticFix` metadata do not
  produce quickfixes.
- Recommended-profile authoring hints include the canonical `init` alias reminder and the
  frontmatter `config` preference; the frontmatter-config rule now carries a migration fix that
  rewrites init/initialize directive config into YAML frontmatter.

## Deferred

- Formatting remains deferred.
