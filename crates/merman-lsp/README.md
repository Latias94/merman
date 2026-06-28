# merman-lsp

`merman-lsp` is the canonical LSP transport for diagnostics, completion, structure-aware
navigation, code actions, semantic tokens, and workspace symbol foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `completionItem/resolve`, `documentSymbol`, `definition`, `references`, `prepareRename`,
  `rename`, `codeAction`, `semanticTokens/full`, and `semanticTokens/range`.
- Advertise editor-agnostic Merman extension requests under `ServerCapabilities.experimental`,
  including `merman/ruleCatalog` and `merman/configSchema`.
- Publish diagnostics from `merman-analysis`, including pull diagnostics.
- Keep document state versioned so stale diagnostics are not republished.
- Provide the first completion surface for diagram structure, directions, shapes, and local
  identifiers with stable text edits.
- Provide fence-local structure and navigation responses for hover, document symbols, definition,
  references, and rename.
- Provide fix-backed quickfix code actions from shared analysis diagnostics.
- Provide parser-backed full/range/delta semantic tokens from shared semantic items.
- Provide workspace symbols from tracked document snapshots.

## Deferred

- Formatting

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion items are snapshot-driven and fence-local instead of raw string insertions.
- First-class family coverage is tracked in `docs/lsp/CAPABILITIES.md`; parse/render support
  outside that matrix is not yet a mature LSP commitment.
