---
type: Skill Contract
status: active
---

# Merman LSP

`merman-lsp` is the canonical LSP transport for diagnostics, completion, structure-aware
navigation, code-action, and semantic-token surfaces. The semantic editor queries are shared with
browser integrations through `merman-editor-core`; editor-core owns protocol-neutral document
snapshots, UTF-16 ranges, completion, symbols, navigation, rename, and semantic-token selection.
`merman-lsp` keeps the LSP lifecycle, capability negotiation, diagnostics publication,
semantic-token delta state, custom requests, and `tower_lsp::lsp_types` projection.

## Responsibilities

- Accept `initialize`, `didOpen`, `didChange`, `didSave`, `didClose`, `completion`, `hover`,
  `completionItem/resolve`, `documentSymbol`, `definition`, `references`, `prepareRename`,
  `rename`, `selectionRange`, `foldingRange`, `codeAction`, `semanticTokens/full`, and
  `semanticTokens/range`.
- Advertise editor-agnostic Merman extension requests under `ServerCapabilities.experimental`,
  including `merman/ruleCatalog` for rule metadata discovery and `merman/configSchema` for
  analysis/lint settings discovery.
- Publish diagnostics from `merman-analysis` and answer standard pull diagnostic requests from the
  same analysis payloads.
- Keep document state versioned so diagnostics from stale analysis snapshots are suppressed before
  publication.
- Project `merman-editor-core` completion, hover, document symbols, selection ranges, folding
  ranges, definition, references, prepare-rename, rename, workspace symbols, and semantic tokens
  into LSP types.
- Provide workspace symbols from tracked document snapshots.
- Preserve parser-backed semantic items from `merman-analysis` so semantic tokens, future lint, and
  code actions can consume payload roles without LSP-local parsing.
- Use entity-only semantic queries for definition, references, prepare-rename, and rename.
- Resolve references and rename through typed semantic groups keyed by semantic kind, not just
  raw names.
- Serve full-document semantic tokens from parser-backed semantic items with stable
  entity/outline/payload role modifiers.
- Serve range and delta semantic-token requests from the same parser-backed token stream.
- Serve quickfix code actions only from `DiagnosticFix` metadata carried by merman diagnostics.
- Stay Markdown-fence aware and UTF-16 correct for plain Mermaid, Markdown, and MDX documents.

## Deferred

- Additional fix-producing lint rules and configuration
- Formatting
- Deeper completion documentation for family-specific syntax variants

## Product Direction

- LSP behavior is driven by parser-backed semantic facts.
- Text-scan results are visible fallback provenance, not a mature capability signal.
- `docs/lsp/CAPABILITIES.md` is the maturity contract. Families outside that matrix may still
  parse or render, but they are not first-class LSP commitments yet.
- The current supported product-family set is first-class in the capability matrix. `error`
  remains an internal fallback diagram rather than a user-facing LSP family.
- Payload-first families such as Info, Pie, Packet, and XY Chart can be mature without exposing
  many rename/reference targets; sparse navigation is a family property, not a server defect.
- Workspace symbols reuse the tracked outline projection from snapshots instead of a separate
  parser path.
- Payload facts should be retained in the analysis index but excluded from completion, outline, and
  rename unless a role explicitly allows projection.
- Payload facts can feed hover, lint, semantic tokens, and code actions without becoming navigation
  or rename targets.
- Code actions are fix-metadata driven; LSP does not invent edits for diagnostics without explicit
  safe fixes.
- Recommended-profile authoring rules such as `merman.authoring.config.prefer_init_directive`,
  `merman.authoring.config.prefer_frontmatter_config`, and
  `merman.authoring.flowchart.explicit_direction` are available through the shared lint
  configuration, but remain opt-in through `recommended` or explicit rule enablement. The
  frontmatter-config rule carries a migration quickfix that rewrites init/initialize directive
  config into YAML frontmatter.
- Mermaid-backed compatibility rules such as
  `merman.compatibility.config.deprecated_flowchart_html_labels` and
  `merman.compatibility.config.deprecated_external_diagram_loading` can be core-profile warnings
  when the pinned Mermaid source or docs expose the same warning. The flowchart htmlLabels rule
  reports deprecated `flowchart.htmlLabels` but does not provide an automatic migration quickfix,
  because moving it to root `htmlLabels` can change Mermaid rendering semantics.
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
- Completion uses editor-core replacement ranges, so header, operator, direction, shape, and node
  completions replace the current token instead of blindly inserting at the cursor.
- Completion items carry stable resolve data and `completionItem/resolve` adds Markdown
  documentation without changing insert text or text edits.
