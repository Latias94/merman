# Merman VS Code Extension

This local extension starts `merman-lsp` inside VS Code so you can exercise diagnostics,
completion, hover, symbols, rename, references, code actions, and semantic tokens against the
current workspace build.

## What it supports

- Mermaid source files: `.mmd`, `.mermaid`
- Markdown-family documents: `markdown`, `.markdown`, `.mdx` Mermaid fences through the same LSP
- Local `merman-lsp` launch through:
  - `target/debug/merman-lsp` when already built
  - `cargo run -p merman-lsp --` as fallback
- Runtime analysis settings forwarded through `initialize` and `workspace/didChangeConfiguration`

## Quick start

1. Build the language server once:

   ```bash
   cargo build -p merman-lsp
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

## Settings

- `merman.server.path`: absolute path to a prebuilt `merman-lsp`
- `merman.server.useCargoRun`: force `cargo run -p merman-lsp --`
- `merman.server.cargoArgs`: extra Cargo flags before `--`
- `merman.server.args`: extra server args after the executable
- `merman.trace.server`: VS Code LSP trace level
- `merman.analysis.*`: analysis/lint settings forwarded to `merman-lsp`

## Packaging

Build and produce a local `.vsix`:

```bash
cd tools/vscode-extension
npm run build
npm run package
```
