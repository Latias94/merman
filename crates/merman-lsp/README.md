# merman-lsp

`merman-lsp` is the canonical LSP transport for diagnostics, completion, and structure-aware
navigation foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `definition`, `references`, `prepareRename`, and `rename`.
- Publish diagnostics from `merman-analysis`.
- Keep document state versioned so stale diagnostics are not republished.
- Provide the first completion surface for diagram structure, directions, shapes, and local
  identifiers with stable text edits.
- Provide fence-local structure and navigation responses for hover, document symbols, definition,
  references, and rename.

## Deferred

- Code actions
- Semantic tokens
- Workspace symbols
- Formatting

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion items are snapshot-driven and fence-local instead of raw string insertions.
