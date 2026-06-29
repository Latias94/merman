# Merman VS Code Extension

Merman for VS Code provides local Mermaid authoring backed by `merman-lsp` and `merman-cli`.
It is designed for parser-backed diagnostics, completion, hover, symbols, rename, references,
code actions, semantic tokens, preview, and export without login, telemetry, cloud sync, or
remote AI.

## What it supports

- Mermaid source files: `.mmd`, `.mermaid`
- Markdown-family documents: `markdown`, `.markdown`, `.mdx` Mermaid fences through the same LSP
- Parser-backed completion, hover, symbols, references, rename, semantic tokens, readable diagnostics,
  and safe quick fixes where Merman analysis provides fix metadata
- Static Mermaid snippets for common diagram skeletons, plus LSP snippets for context-sensitive helper
  inserts and diagram templates
- Preview panel for the active `.mmd`, `.mermaid`, or current Markdown/MDX Mermaid fence, with scoped
  diagnostics, quick-fix entry points, source pinning, multi-fence selection, zoom, theme/background
  inspection controls, and local SVG copy
- Local SVG and PNG export for Mermaid files and the active Markdown/MDX Mermaid fence, including
  an export picker with open-after-export options
- Local runtime launch through one shared resolver:
  - packaged `bin/<platform>-<arch>/merman-lsp` and `merman-cli`
  - user-configured absolute executable paths
  - workspace `target/debug` binaries for extension development
  - explicit Cargo development fallback when enabled in settings
- Runtime analysis settings forwarded through `initialize` and `workspace/didChangeConfiguration`
- Rule catalog and config schema inspection commands backed by the custom LSP requests

## Quick start

### Packaged install

Install a VSIX or Marketplace build that includes the platform runtime binaries. No Rust toolchain
is required for normal use.

### Extension development

1. Build local debug binaries once:

   ```bash
   cargo build -p merman-lsp -p merman-cli
   ```

2. Install extension dependencies and build:

   ```bash
   cd tools/vscode-extension
   npm install
   npm run build
   ```

3. Launch an extension development window:

   ```bash
   code --extensionDevelopmentPath="$PWD/tools/vscode-extension"
   ```

4. Open a `.mmd`, `.mermaid`, `.md`, `.markdown`, or `.mdx` file and edit a Mermaid diagram.

## Commands

- `Merman: Restart Language Server`
- `Merman: Open Preview`
- `Merman: Export...`
- `Merman: Export SVG`
- `Merman: Export PNG`
- `Merman: Copy SVG`
- `Merman: Copy PNG` (uses the local platform clipboard when available, otherwise falls back to PNG export)
- `Merman: Show Rule Catalog`
- `Merman: Show Config Schema`

The extension intentionally coexists with VS Code's built-in Mermaid support in Markdown preview.
Use Merman for semantic editing, `.mmd` files, fence-aware diagnostics, local preview inspection,
and export; it does not replace VS Code's Markdown preview renderer.

## Settings

- `merman.server.path`: absolute path to a prebuilt `merman-lsp`
- `merman.server.useCargoRun`: development-only fallback through `cargo run -p merman-lsp --`
- `merman.server.cargoArgs`: development-only Cargo flags before `--`
- `merman.server.args`: extra server args after the executable
- `merman.cli.path`: absolute path to a prebuilt `merman-cli`
- `merman.cli.useCargoRun`: development-only fallback through `cargo run -p merman-cli --`
- `merman.cli.cargoArgs`: development-only Cargo flags before `--`
- `merman.trace.server`: VS Code LSP trace level
- `merman.analysis.*`: analysis/lint settings forwarded to `merman-lsp`

## Packaging

Build release binaries, copy them into the extension runtime folder, and produce a local `.vsix`:

```bash
cargo build --release -p merman-lsp -p merman-cli
cd tools/vscode-extension
npm run build
npm run prepare:binaries
npm run package
```

`npm run package` expects `npm run prepare:binaries` to have populated `bin/<platform>-<arch>/`.
The development-only Cargo fallbacks are disabled by default so a packaged VSIX does not silently
depend on a Rust workspace.
