# Merman integrations

Choose a surface by host and workflow, not by Merman's internal crate layout. Browser packages are
browser-only, the native Node.js package is intentionally narrow, and lint-only hosts should not
carry rendering or editor-session code by default.

## Choose by host

| Host or task | Start with | Boundary |
| --- | --- | --- |
| Browser SVG rendering | [`@mermanjs/web-render`](../../platforms/web/packages/render/README.md) | Complete SVG capability without analysis, editor, or ASCII workflows |
| Browser application with several workflows | [`@mermanjs/web`](../../platforms/web/packages/full/README.md) | Rendering, analysis, ASCII, and editor APIs in one browser artifact |
| Browser or Worker analysis | [`@mermanjs/web-analysis`](../../platforms/web/packages/analysis/README.md) | Detection, validation, facts, and diagnostics without rendering |
| Native Node.js or static-site rendering | [`@mermanjs/node`](../../platforms/node/packages/node/README.md) | Experimental Node.js 22+ deterministic SVG package |
| Shell, CI, or a Node.js subprocess | [`merman-cli`](../../crates/merman-cli/README.md) | Native rendering, linting, Markdown batches, export, and JSON output |
| Generic editor or LSP client | [`merman-lsp`](../../crates/merman-lsp/README.md) | Language intelligence only; preview rendering stays separate |
| VS Code | [Merman VS Code extension](../../tools/vscode-extension/README.md) | Local LSP plus optional preview, export, diagnostics, and source actions |
| Rust application | [`merman`](https://docs.rs/merman), [`merman-analysis`](../../crates/merman-analysis/README.md), or [`merman-editor-core`](../../crates/merman-editor-core/README.md) | In-process typed APIs selected by capability |

The browser and Node.js package groups are published on npm's `alpha` dist-tag. Pin an exact
version when reproducible installs matter, and check the [package surface guide](../release/PACKAGE_SURFACES.md)
for the current delivery boundary.

## Browser analysis

Use `@mermanjs/web-analysis` for browser linting, diagram detection, and metadata workflows. The
document URI is required because its extension selects standalone Mermaid, Markdown, or MDX source
modeling.

```ts
import { analyzeDocument, initMerman } from "@mermanjs/web-analysis";

await initMerman();

const markdownSource =
  "# Diagram\n\n```mermaid\nflowchart TD\n  A -->\n```\n";
const result = analyzeDocument(
  markdownSource,
  "file:///workspace/README.md",
  { lint: { profile: "recommended" } },
);
```

Diagnostics retain host-document spans, related locations, help text, and fix edits. Use the
complete `@mermanjs/web` package when the same browser realm also needs several of rendering,
ASCII, analysis, and editor sessions.

Browser packages require a browser main thread or Web Worker. Do not load them from Node.js, SSR,
or a Node-based lint runner through a compatibility shim.

## CLI and CI

Invoke `merman-cli` as a child process when a native process boundary fits the host. Its JSON output
is suitable for adapters and CI reporting.

```sh
merman-cli lint --format json diagram.mmd
merman-cli lint --markdown --format json README.md
```

## Native Node.js and SSG

`@mermanjs/node` provides in-process deterministic SVG for supported Node.js hosts. Install only
the root loader package; it selects an exact-version native platform package. The package has no
browser fallback, postinstall download, math, binary export, analysis, or ASCII surface.

See the [Node.js package guide](../../platforms/node/packages/node/README.md) for lifecycle,
concurrency, supported platforms, and capability boundaries. Use the [CLI guide](../../crates/merman-cli/README.md)
when a subprocess integration is a better fit.

## Editors

Use [`merman-lsp`](../../crates/merman-lsp/README.md) for completion, hover, document symbols,
navigation, rename, selection and folding ranges, Tree-sitter syntax highlighting, diagnostics, and
fix-backed code actions. The LSP does not render previews; let the editor's Markdown preview,
another Mermaid preview extension, or `merman-cli` own rendering.

The [VS Code extension](../../tools/vscode-extension/README.md) combines the local LSP with optional
preview and export. Disable only the overlapping surface when another tool owns it:

```json
{
  "merman.sourceActions.enabled": false,
  "merman.diagnostics.enabled": false
}
```

These settings leave the language server running. See [VS Code coexistence](vscode-coexistence.md)
for common preview and diagnostics combinations.

## Lint ownership

External tools can use Merman diagnostics as evidence without transferring all lint policy to
Merman. Keep repository discovery, ignore rules, CI formatting and exit policy, Mermaid.js
fallback, and non-Merman style rules in the integrating tool when those are part of its contract.

Merman-owned rules retain their `merman.*` IDs. Do not map unrelated markdownlint, remark,
textlint, or project-specific rules into that namespace. The [lint interop guide](lint-interop.md)
describes rule ownership, adapter shape, spans, and fixes.

## Non-goals

Merman does not provide a Mermaid.js-authoritative runtime fallback, replace every Mermaid lint
ecosystem, require users to disable VS Code's built-in Mermaid Markdown preview, or treat a
browser-only WASM package as a Node.js transport.
