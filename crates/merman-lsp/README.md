# merman-lsp

`merman-lsp` is the canonical LSP transport for diagnostics, completion, structure-aware
navigation, and workspace symbol foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `definition`, `references`, `prepareRename`, and `rename`.
- Publish diagnostics from `merman-analysis`.
- Keep document state versioned so stale diagnostics are not republished.
- Provide the first completion surface for diagram structure, directions, shapes, and local
  identifiers with stable text edits.
- Provide fence-local structure and navigation responses for hover, document symbols, definition,
  references, and rename.
- Provide workspace symbols from tracked document snapshots.

## Deferred

- Code actions
- Semantic tokens
- Formatting

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion items are snapshot-driven and fence-local instead of raw string insertions.
