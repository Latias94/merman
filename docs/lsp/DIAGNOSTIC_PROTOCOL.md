---
type: Skill Contract
status: active
---

# Diagnostic Protocol

`merman-lsp` is the canonical LSP transport for diagnostics, completion, and fix-backed code
actions. It projects `merman-analysis` payloads into LSP diagnostics without adding a second
analysis path, and serves both standard push diagnostics and LSP 3.17 pull diagnostics.

## Canonical rules

- Source of truth for analysis diagnostics: `merman-analysis::AnalysisPayload`
- Ownership: core emits structured parse diagnostics; analysis owns canonical merge, fallback,
  and recovery policy; editor-core and LSP only project that payload.
- Transport and server service: `tower-lsp-server`; LSP wire types come from its maintained
  `ls_types` re-export.
- Stdio framing: Merman owns frame dispatch so rejected requests cannot consume later pipelined
  messages; headers are limited to 8 KiB and message bodies to 32 MiB.
- Coordinate system: UTF-16 LSP positions
- Markdown fences: remapped to the host document URI and range
- Visible Problems code: string analysis rule id such as `merman.parse.diagram_parse`; numeric
  analysis status and auxiliary fields such as `codeName` and `diagramType` remain in
  `AnalysisPayload` and are not copied to LSP `Diagnostic.data`. When diagnostic data is negotiated,
  analysis diagnostics carry only the server-owned diagnostic id and current document version used
  to validate code actions.

## Compatibility

- Plain Mermaid documents publish diagnostics against the file URI directly.
- Markdown/MDX documents publish diagnostics against the containing document URI.
- `merman.lsp.document_sync_lost` is the one LSP-owned protocol-integrity diagnostic. It reports
  that an invalid incremental edit or a ranged edit after source discard left the server without
  authoritative text. It is not an analysis rule or rule-catalog entry, and its negotiated data
  contains only the document version.
- `merman.resource.document_diagrams_exceeded` remains an analysis-owned resource diagnostic.
  The LSP retains authoritative Markdown/MDX text while analysis is rejected, projects the
  canonical payload for both push and pull diagnostics, and continues accepting ranged edits. It
  must not translate this state into `merman.lsp.document_sync_lost`.

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
- Document symbols are wired from tracked document snapshots. Workspace symbols are not advertised:
  `ServerCapabilities.workspace_symbol_provider` is `None`, and `workspace/symbol` requests return
  JSON-RPC `MethodNotFound`.
- Core config diagnostics include source-backed Mermaid compatibility warnings such as deprecated
  directive usage of `flowchart.htmlLabels` (diagnostic-only because automatic migration can
  change rendering semantics) and
  deprecated external diagram loading config; diagnostics without `DiagnosticFix` metadata do not
  produce quickfixes.
- Recommended-profile authoring hints include the canonical `init` alias reminder and the
  frontmatter `config` preference; the frontmatter-config rule now carries a migration fix that
  rewrites init/initialize directive config into YAML frontmatter.
- A document-wide `DiagnosticFix` may be attached to several diagnostics. Analysis keeps the edit
  slice shared, and the LSP code-action path deduplicates requested owners by that shared identity
  before materializing the server-owned workspace edit. Diagnostic schema 1 remains unchanged.

## Request Interleaving

Diagnostics are computed through typed `LanguageSession` operations. Each operation captures a
document/analyzer ticket under the short-lived session mutex, projects analysis payloads without
holding that mutex, and commits only while the document epoch and diagnostic configuration
generation are still current. Before sending push diagnostics, the server
checks that the captured context remains current;
stale contexts observed before the final publish attempt are suppressed. A notification already
handed to the client transport is outside this cancellation boundary.

For pull diagnostics, `textDocument/diagnostic` performs the same currentness check after analysis.
If the captured context became stale, the server recomputes from the latest context with a bounded
retry loop of up to three attempts and returns that result. Pull-mode configuration changes
invalidate client caches with `workspace/diagnostic/refresh` when the client advertises refresh
support, and they do not also push open-document diagnostics.

## Deferred

- Formatting remains deferred.
