# merman-lsp

`merman-lsp` is the canonical LSP transport for Merman diagnostics and protocol-neutral editor
intelligence. It validates the LSP projection while analysis and editor-core own the underlying
language behavior.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `completionItem/resolve`, `documentSymbol`, `definition`, `references`, `prepareRename`,
  `rename`, `selectionRange`, `foldingRange`, `codeAction`, `semanticTokens/full`, and
  `semanticTokens/range`.
- Advertise editor-agnostic Merman extension requests under `ServerCapabilities.experimental`,
  including `merman/ruleCatalog` and `merman/configSchema`.
- Publish diagnostics from `merman-analysis`, including document pull diagnostics.
- Keep document state versioned so diagnostics from stale analysis snapshots are suppressed before
  publication.
- Project `merman-editor-core` completion, hover, document symbols, selection ranges, folding
  ranges, workspace symbols, definition, references, prepare-rename, rename, and semantic-token
  responses into LSP types.
- Provide fix-backed quickfix code actions from shared analysis diagnostics.
- Reject `workspace/diagnostic` while unopened workspace-file scanning has no owner; tracked
  document snapshots still support workspace symbols.

## Deferred

- Formatting

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Language features remain editor-core-driven; the LSP layer converts URI/range/type shapes and
  handles request lifecycle.
- First-class family coverage is tracked in `docs/lsp/CAPABILITIES.md`; parse/render support
  outside that matrix is not yet a mature LSP commitment.
