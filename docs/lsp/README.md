---
type: Skill Contract
status: active
---

# Merman LSP

`merman-lsp` is the canonical LSP transport for diagnostics and completion foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, and `completion`.
- Publish diagnostics from `merman-analysis`.
- Keep document state versioned so stale diagnostics are not republished.
- Provide the first completion surface for diagram structure, directions, shapes, and local identifiers.

## Deferred

- Hover
- Go to definition
- Rename
- Code actions
- Semantic tokens
- Workspace symbols
- Formatting
- Completion resolution payloads

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion now uses snapshot-derived replacement ranges, so header, operator, direction, shape,
  and node completions replace the current token instead of blindly inserting at the cursor.
