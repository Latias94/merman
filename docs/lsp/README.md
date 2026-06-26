---
type: Skill Contract
status: active
---

# Merman LSP

`merman-lsp` is the canonical LSP transport for diagnostics, completion, structure-aware
navigation, code-action, and semantic-token foundations.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `completionItem/resolve`, `documentSymbol`, `definition`, `references`, `prepareRename`,
  `rename`, `codeAction`, `semanticTokens/full`, and `semanticTokens/range`.
- Advertise editor-agnostic Merman extension requests under `ServerCapabilities.experimental`,
  including `merman/ruleCatalog` for rule metadata discovery and `merman/configSchema` for
  analysis/lint settings discovery.
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
- Delta semantic tokens
- Formatting
- Deeper completion documentation for family-specific syntax variants

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
- Fix-backed authoring rules such as `merman.authoring.config.prefer_init_directive` and
  `merman.authoring.flowchart.explicit_direction` are available through the shared lint
  configuration, but remain opt-in through `recommended` or explicit rule enablement.
- Full-document, range, and delta semantic tokens are role-aware and parser-backed; configuration
  changes trigger `workspace/semanticTokens/refresh` when the client advertises refresh support,
  and delta requests reuse cached snapshot token state when the previous result id matches.
- Capability maturity is tracked in `CAPABILITIES.md`.
- Merman-specific custom requests are documented in `EXTENSION_PROTOCOL.md`; editor plugins should
  feature-detect those requests, use the config schema for settings completion/validation hints,
  and keep UI outside the LSP server.

## Notes

- Plain Mermaid files and Markdown/MDX fenced Mermaid blocks are both supported.
- Diagnostics remain analysis-driven; the LSP layer does not reimplement parse or render rules.
- Completion uses snapshot-derived replacement ranges, so header, operator, direction, shape, and
  node completions replace the current token instead of blindly inserting at the cursor.
- Completion items carry stable resolve data and `completionItem/resolve` adds Markdown
  documentation without changing insert text or text edits.
