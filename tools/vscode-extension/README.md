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
  - explicit Cargo development fallback when enabled in settings
- Runtime analysis settings forwarded through `initialize` and `workspace/didChangeConfiguration`
- Rule catalog and config schema inspection commands backed by the custom LSP requests

## Quick start

### Packaged install

Install a VSIX or Marketplace build that includes the platform runtime binaries. No Rust toolchain
is required for normal use.

### Extension development

1. Install extension dependencies and build:

   ```bash
   cd tools/vscode-extension
   npm install
   npm run build
   ```

2. Launch an extension development window:

   ```bash
   code --extensionDevelopmentPath="$PWD"
   ```

3. For workspace development, either prepare packaged binaries under `bin/<platform>-<arch>/`
   or enable the Cargo fallbacks in the extension development window:

   ```json
   {
     "merman.server.useCargoRun": true,
     "merman.cli.useCargoRun": true
   }
   ```

   Cargo fallback runs `cargo run -p merman-lsp --` and `cargo run -p merman-cli --` from a
   trusted workspace. Use `merman.server.cargoArgs` and `merman.cli.cargoArgs` for extra Cargo
   flags such as `--profile` or `--features`.

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

The extension intentionally coexists with VS Code's built-in Mermaid support in Markdown preview,
external Mermaid preview extensions, and project lint tools. Use Merman for semantic editing,
`.mmd` files, optional fence-aware diagnostics, local preview inspection, and export; it does not
replace VS Code's Markdown preview renderer or repository lint policy.

Preview SVG uses the same generated DOM safety policy as `@mermanjs/web`; see
[`docs/security/RENDERING_SECURITY.md`](https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md)
for host responsibilities when SVG is inserted into a browser or editor webview.

The packaged extension is a local capability surface over `merman-lsp` and `merman-cli`, not a
claim that every project should adopt an LSP-first Mermaid workflow. Users can keep preview/export
only, language intelligence without diagnostics, or the full local authoring stack.

## Coexistence modes

Use Merman only for local preview and export without starting the language server:

```json
{
  "merman.languageIntelligence.enabled": false
}
```

Keep Merman language intelligence while another preview extension owns editor actions:

```json
{
  "merman.sourceActions.enabled": false
}
```

Keep Merman completion, hover, symbols, references, rename, and semantic tokens while another linter
owns VS Code Problems:

```json
{
  "merman.diagnostics.enabled": false
}
```

JavaScript lint tools that want Merman parser-backed evidence can use `@mermanjs/web`
`analyzeDocument(source, options, uri)` without adopting the LSP. See
`docs/integrations/` for adapter guidance and coexistence examples.

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

Merman contributes native VS Code settings grouped under Runtime, Language Intelligence, Analysis,
Preview and Export, and Development.

- `merman.server.path`: absolute path to a prebuilt `merman-lsp`
- `merman.server.useCargoRun`: development-only fallback through `cargo run -p merman-lsp --`
- `merman.server.cargoArgs`: development-only Cargo flags before `--`
- `merman.server.args`: extra server args after the executable; restricted setting that requires VS Code Workspace Trust
- `merman.cli.path`: absolute path to a prebuilt `merman-cli`
- `merman.cli.useCargoRun`: development-only fallback through `cargo run -p merman-cli --`
- `merman.cli.cargoArgs`: development-only Cargo flags before `--`
- `merman.trace.server`: VS Code LSP trace level
- `merman.languageIntelligence.enabled`: start local `merman-lsp` language intelligence
- `merman.diagnostics.enabled`: publish Merman diagnostics to VS Code Problems
- `merman.sourceActions.enabled`: show source-scoped Merman CodeLens actions
- `merman.preview.diagramTheme`: default theme for new preview panels
- `merman.preview.displayMode`: default `svg`, `ascii`, or `unicode` preview mode
- `merman.preview.background`: default `paper`, `transparent`, or `dark` preview background
- `merman.analysis.*`: analysis/lint settings forwarded to `merman-lsp`

## Packaging

Build release binaries, copy them into the extension runtime folder, and produce a local `.vsix`:

```bash
cargo build --release -p merman-lsp -p merman-cli
cd tools/vscode-extension
npm run build
npm run prepare:binaries
target="$(node -p "process.platform + '-' + process.arch")"
npm run package -- --target "$target" --out "merman-vscode-${target}.vsix"
```

`npm run package` expects `npm run prepare:binaries` to have populated `bin/<platform>-<arch>/`,
and the `--target` argument should match that platform key. Use this wrapper instead of invoking
`vsce package` directly: `package.json` keeps the Marketplace-compatible manifest version such as
`0.8.0`, while the wrapper reads the workspace release version and passes `--pre-release` when that
release version is a SemVer prerelease such as `0.8.0-alpha.3`.
The development-only Cargo fallbacks are disabled by default so a packaged VSIX does not silently
depend on a Rust workspace.

The GitHub Actions workflow `.github/workflows/vscode-extension.yml` runs the same package smoke on
Linux, macOS, and Windows and uploads platform-specific VSIX artifacts named from the detected Node
runtime platform, such as:

- `merman-vscode-linux-x64.vsix`
- `merman-vscode-darwin-arm64.vsix`
- `merman-vscode-darwin-x64.vsix`
- `merman-vscode-win32-x64.vsix`

Before publishing a VSIX to the Marketplace, verify that `package.json` has the intended
`publisher`, stable manifest `version`, `preview` status, repository links, and changelog entry,
then run `npm run verify:vsix -- --vsix <file> --platform <target> --target <target>`. The verifier
checks the VSIX package manifest version and the pre-release marker against the workspace release
version. The extension is packaged with runtime binaries for the platform that built it, so publish
or distribute the matching artifact for the target host.
