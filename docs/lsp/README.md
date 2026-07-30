---
type: Skill Contract
status: active
---

# Merman LSP Maintainer Architecture

> This is a maintainer-facing architecture contract. Users installing or embedding the server should start with the [`merman-lsp` crate guide](../../crates/merman-lsp/README.md). Plugin authors should use the [extension protocol](EXTENSION_PROTOCOL.md) and [capability matrix](CAPABILITIES.md) as their public contracts.

## Scope

`merman-lsp` is the canonical LSP adapter for Merman diagnostics and editor intelligence. It does not parse Mermaid through an LSP-specific grammar and does not render previews.

`merman-editor-core` owns protocol-neutral document snapshots, UTF-16 ranges, completion, hover, document symbols, selection and folding ranges, navigation, rename, and semantic-token planning. `merman-lsp` owns:

- LSP initialization, lifecycle, capability negotiation, and `tower_lsp_server::ls_types` projection;
- synchronous message admission plus ordered document and configuration transactions;
- generation acquisition, singleflight execution, weighted caching, stale-result suppression, and diagnostics publication;
- semantic-token result IDs and delta state;
- standard request handlers and Merman-specific extension requests; and
- client effects, refresh coordination, cancellation, and bounded session shutdown.

Preview, export, and editor UI stay in their host integrations.

## Advertised Protocol Surface

The server handles document lifecycle notifications and advertises the supported subset of:

- completion and `completionItem/resolve`;
- hover, document symbols, selection ranges, and folding ranges;
- definition, references, prepare rename, and rename;
- push diagnostics or standard pull diagnostics according to client capabilities;
- fix-backed quick-fix code actions; and
- full-document, range, and delta semantic tokens when the client supports them.

Capabilities are negotiated. Code actions require compatible diagnostic-data and quick-fix literal support, while semantic-token legends and modifiers are projected onto the client's supported set.

Workspace symbols are not part of the current protocol surface: `ServerCapabilities.workspace_symbol_provider` is `None`, and `workspace/symbol` requests return JSON-RPC `MethodNotFound`.

The server advertises editor-agnostic Merman requests under `ServerCapabilities.experimental.merman`, including `merman/ruleCatalog` and `merman/configSchema`. Clients must feature-detect these requests instead of assuming that a particular server version exposes them.

## Document And State Model

Plain Mermaid files and Mermaid fences in Markdown and MDX use the same typed `DocumentSnapshot`/`FenceTextIndex` model. Host-document positions remain UTF-16 correct, including diagnostics, fixes, completion edits, navigation, rename, and semantic tokens.

`MermanLspService` synchronously admits each JSON-RPC message before transport futures can run concurrently. State mutations are ordered by arrival, while reads wait for every earlier mutation to commit or abort. One private `LanguageSession` versions source and analyzer configuration independently and owns generation acquisition, singleflight execution, weighted LRU entries, guarded commits, client effects, refresh coordination, cancellation, and endpoint lifetime. Typed session operations capture a short-lived state ticket, perform parsing or projection outside the session mutex, then commit only if the captured epoch and configuration remain current. Handlers do not own transaction choreography.

The service and client socket are two endpoints of that same session. Dropping either endpoint, receiving `exit`, or closing the socket stream terminates the shared lifecycle and cancels child work exactly once. The bundled stdio transport keeps cancellation and `exit` reachable under ordinary-handler saturation and applies one absolute deadline to handler, write, flush, and output-shutdown drain work.

- Push diagnostics re-check currentness immediately before publication.
- Pull diagnostics retry bounded stale computations against the latest context.
- Semantic-token state is committed only for the captured current snapshot.
- A matching previous result ID can produce a delta; a stale ID falls back to full tokens.
- Snapshot-affecting configuration clears snapshot-dependent state.
- Diagnostic-only rule changes refresh diagnostics without rebuilding semantic snapshots.

Initialization and workspace settings can provide deterministic `fixed_today` and `fixed_local_offset_minutes` values. The LSP does not expose a native runtime selector or forward the facade's `system-*` Cargo features.

## Semantic Ownership

Language behavior is driven by parser-backed facts from `merman-analysis` and queries from `merman-editor-core`:

- definition, references, prepare rename, and rename operate on typed entity groups rather than raw text matches;
- payload facts may feed hover, lint, semantic tokens, and code actions without becoming navigation or rename targets;
- code actions use explicit `DiagnosticFix` metadata and never invent edits for diagnostics without a safe fix; and
- completion uses editor-core replacement ranges, while resolve adds documentation without changing the selected insert edit.

`Unavailable` provenance means no parser-backed body facts exist. Unknown or unrecoverable body text therefore receives no guessed body symbols, navigation, rename, or semantic tokens. Source-start diagram headers and templates remain available from the static family catalog.

The LSP consumes typed snapshots directly; it does not serialize and decode `AnalysisFactsPayload` for normal requests. The separately exposed binding facts payload and diagnostics payload are independent schema-v1 wire contracts, not LSP document versions or Mermaid grammar IDs.

Family-level maturity is defined only by [CAPABILITIES.md](CAPABILITIES.md). A family may parse or render without being a first-class LSP commitment. Payload-first families can be mature while intentionally exposing few rename or reference targets, and `error` remains an internal fallback family.

## Diagnostics And Extensions

Diagnostics stay analysis-driven; the LSP layer does not reimplement parser, compatibility, resource, or authoring rules. Rule metadata, configuration discovery, diagnostic payload behavior, and fix transport are documented in:

- [Diagnostic protocol](DIAGNOSTIC_PROTOCOL.md)
- [Extension protocol](EXTENSION_PROTOCOL.md)
- [Integration and coexistence guide](../integrations/README.md)

Configuration changes request `workspace/semanticTokens/refresh` or `workspace/diagnostic/refresh` only when the client advertises the corresponding support.

## Deferred

- Advertised workspace symbols and ownership of unopened workspace-file discovery
- Formatting
- Additional source-backed, fix-producing rules
- Deeper completion documentation for family-specific syntax variants
