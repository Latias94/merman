# Merman Integrations

Merman integrations are deliberately split by surface:

- `merman-analysis` owns Merman diagnostics, rule metadata, source spans, related locations, and fixes.
- `@mermanjs/web` exposes analysis payloads to JavaScript tools that want parser-backed evidence without LSP.
- `merman-lsp` projects protocol-neutral editor intelligence into LSP.
- The VS Code extension wires local language features, preview, export, and optional diagnostics for users who want them.

External Mermaid lint and preview tools do not need to adopt Merman's analysis engine to coexist. Treat Merman as one local capability provider: share the analysis payload when useful, or keep independent lint/preview policy and disable overlapping editor surfaces.

## Supported Modes

### Merman Language Intelligence With External Preview

Use the VS Code extension for completion, hover, symbols, references, rename, semantic tokens, and optional quick fixes. Let VS Code's built-in Markdown preview or another Mermaid preview extension own rendering.

Recommended settings:

```json
{
  "merman.sourceActions.enabled": false
}
```

### External Lint Owns Problems

Use markdownlint, remark, textlint, `mermaid-lint`, or another project policy tool for Problems and CI. Keep Merman language features available without duplicate diagnostics.

Recommended settings:

```json
{
  "merman.diagnostics.enabled": false
}
```

### External Lint Consumes Merman Analysis

JavaScript lint tools can call `@mermanjs/web` `analyzeDocument(source, options, uri)` for standalone `.mmd`, Markdown, or MDX documents. This returns Merman diagnostics with source descriptors, host-document spans, related locations, help text, and fix edits.

Use this when Merman analysis is useful evidence. Keep your own file discovery, CI output format, Mermaid.js fallback, and non-Merman style policy when those are part of your tool's contract.

## Non-Goals

Merman does not provide a Mermaid.js-authoritative runtime fallback, a replacement for every Mermaid lint rule ecosystem, or a requirement that users disable VS Code's built-in Mermaid Markdown preview.
