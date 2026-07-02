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
- Ownership: core emits structured parse diagnostics; analysis owns canonical merge, fallback,
  and recovery policy; editor-core and LSP only project that payload.
- Transport: `tower-lsp`
- Coordinate system: UTF-16 LSP positions
- Markdown fences: remapped to the host document URI and range
- Visible Problems code: string analysis rule id such as `merman.parse.diagram_parse`; numeric
  analysis status code and camelCase metadata such as `codeName` and `diagramType` remain only in
  diagnostic `data`.

## Compatibility

- Plain Mermaid documents publish diagnostics against the file URI directly.
- Markdown/MDX documents publish diagnostics against the containing document URI.

## Current Surface

- Client font metrics, rendering, and HTML label behavior are not part of the LSP contract.
- Completion covers diagram structure, directions, operators, shapes, directives, and local
  identifiers with stable replacement edits.
- Hover, selection ranges, Markdown fence folding ranges, go to definition, references,
  prepare-rename, rename, full-document semantic tokens, range/delta semantic tokens, and
  fix-backed code actions are wired.
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

## Request Interleaving

Diagnostics are computed from captured document/analyzer contexts so the server can release the
document-store lock while projecting analysis payloads. Before sending push diagnostics, the server
checks that the captured document epoch and diagnostic configuration generation are still current;
stale contexts observed before the final publish attempt are suppressed. A notification already
handed to the client transport is outside this cancellation boundary.

For pull diagnostics, `textDocument/diagnostic` performs the same currentness check after analysis.
If the captured context became stale, the server recomputes once from the latest context and returns
that result. Pull-mode configuration changes invalidate client caches with
`workspace/diagnostic/refresh` when the client advertises refresh support, and they do not also push
open-document diagnostics.

## Deferred

- Formatting remains deferred.
