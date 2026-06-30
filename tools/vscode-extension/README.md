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
- Source-scoped CodeLens actions above Mermaid files and Markdown/MDX Mermaid fences for preview
  and a compact `Export / Copy` menu
- Preview panel for the active `.mmd`, `.mermaid`, or current Markdown/MDX Mermaid fence, with scoped
  diagnostic status, multi-fence selection, zoom, display mode/theme/background controls, and local
  SVG copy/export controls
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

Mermaid files and Markdown/MDX Mermaid fences also show low-noise CodeLens source actions for
`Preview` and `Export / Copy`. `Export / Copy` opens export and copy commands for the same stable source id.
Markdown fence actions carry that stable fence id, so they do not retarget when the cursor moves
before the command runs.

The extension intentionally coexists with VS Code's built-in Mermaid support in Markdown preview.
Use Merman for semantic editing, `.mmd` files, fence-aware diagnostics, local preview inspection,
and export; it does not replace VS Code's Markdown preview renderer.

## Preview behavior

The preview panel keeps a stable webview shell while diagram content, compact diagnostic status,
source choices, and editor selection updates are delivered through VS Code webview messages. Moving
the cursor inside the currently rendered source does not rerender the diagram or reset pan/zoom
state.

Detailed diagnostics and fixes belong to VS Code's native Problems, editor underline, hover, and
quick-fix lightbulb surfaces. The preview status is a secondary navigation aid to the first
diagnostic in the active source, not a duplicate Problems list.

The preview does not provide its own pin controls, account/sync controls, AI repair controls, or a
second quick-fix panel. Source selection is automatic for Mermaid files and current Markdown/MDX
fences, with an explicit source picker only when a document has multiple Mermaid fences.

When edits require a rerender, the previous SVG stays visible while the new render runs. If rendering
fails, the old SVG remains inspectable and the error appears as an overlay. Zoom is applied by
resizing the SVG surface from its `viewBox` instead of scaling the whole canvas layer, so the preview
continues to inspect vector output rather than a PNG snapshot.

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
