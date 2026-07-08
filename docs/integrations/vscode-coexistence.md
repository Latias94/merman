# VS Code Coexistence

Merman for VS Code is a local Mermaid authoring tool. It can run beside VS Code's built-in Mermaid Markdown preview, Mermaid preview extensions, and external Markdown or Mermaid linters.

## Surface Settings

```json
{
  "merman.diagnostics.enabled": true,
  "merman.sourceActions.enabled": true
}
```

- `merman.diagnostics.enabled` controls whether Merman diagnostics are published to VS Code Problems.
- `merman.sourceActions.enabled` controls Merman CodeLens rows above `.mmd` files and Markdown/MDX Mermaid fences.

Neither setting disables the language server. Completion, hover, document symbols, workspace symbols, references, rename, semantic tokens, rule catalog, and config schema requests remain available while the server is running.

## Common Modes

### Keep Built-In Markdown Preview

VS Code 1.121 and later include Mermaid rendering in Markdown preview and notebooks. If that preview is enough, hide Merman source CodeLens actions and use Merman only for language intelligence:

```json
{
  "merman.sourceActions.enabled": false
}
```

### Let Another Linter Own Problems

If markdownlint, remark, textlint, or a Mermaid-specific lint extension owns Problems output, suppress Merman Problems while keeping language features:

```json
{
  "merman.diagnostics.enabled": false
}
```

The extension filters diagnostics at the VS Code language-client layer. The LSP still serves language requests and can still provide rule catalog/config schema metadata.

### Use Merman Preview Explicitly

Leave source actions enabled when users want source-scoped preview/export/copy commands for `.mmd` files and Markdown/MDX fences. Merman preview is a local inspection and export surface; it is not required for Markdown preview rendering.

## What Merman Does Not Claim

Merman does not replace VS Code's Markdown preview renderer, force users to disable third-party preview extensions, or take over repository-wide lint policy. It provides local language intelligence and optional Merman diagnostics where users want that ownership.
