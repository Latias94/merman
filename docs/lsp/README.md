---
type: Skill Contract
status: active
---

# Merman LSP

`merman-lsp` is the canonical LSP transport for diagnostics, completion, structure-aware
navigation, code-action, and semantic-token foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `documentSymbol`, `definition`, `references`, `prepareRename`, `rename`, `codeAction`, and
  `semanticTokens/full`.
- Publish diagnostics from `merman-analysis`.
- Keep document state versioned so stale diagnostics are never republished.
- Provide the first completion surface for diagram structure, directions, shapes, and local
  identifiers.
- Provide fence-local structure and navigation responses for hover, document symbols, definition,
  references, and rename.
- Provide workspace symbols from tracked document snapshots.
- Preserve parser-backed semantic items from `merman-analysis` so semantic tokens, future lint, and
  code actions can consume payload roles without LSP-local parsing.
- Use entity-only semantic queries for definition, references, prepare-rename, and rename.
- Resolve references and rename through typed semantic groups keyed by semantic kind, not just
  raw names.
- Serve full-document semantic tokens from parser-backed semantic items with stable
  entity/outline/payload role modifiers.
- Serve quickfix code actions only from `DiagnosticFix` metadata carried by merman diagnostics.
- Stay Markdown-fence aware and UTF-16 correct for plain Mermaid, Markdown, and MDX documents.

## Deferred

- Additional fix-producing lint rules and configuration
- Range and delta semantic tokens
- Formatting
- Completion resolution payloads

## Product Direction

- LSP behavior is driven by parser-backed semantic facts.
- Raw-text scans are migration shims, not the target architecture.
- Workspace symbols reuse the tracked outline projection from snapshots instead of a separate
  parser path.
- Payload facts should be retained in the analysis index but excluded from completion, outline, and
  rename unless a role explicitly allows projection.
- Payload facts can feed hover, lint, semantic tokens, and code actions without becoming navigation
  or rename targets.
- Code actions are fix-metadata driven; LSP does not invent edits for diagnostics without explicit
  safe fixes.
- The first fix-backed lint rule is `merman.config.prefer_init_directive`, which replaces the
  Mermaid directive alias `initialize` with the canonical `init` keyword.
- Full-document semantic tokens are role-aware and parser-backed; range/delta support should wait
  until snapshot invalidation and client capability negotiation are explicit.
- Capability maturity is tracked in `CAPABILITIES.md`.

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion uses snapshot-derived replacement ranges, so header, operator, direction, shape, and
  node completions replace the current token instead of blindly inserting at the cursor.
