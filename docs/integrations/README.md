# Merman Integrations

Choose an integration by runtime and workflow. Merman's browser packages are not Node.js packages, and a lint-only host should not carry rendering, ASCII, or editor-session code by default.

| Host | Recommended surface | Boundary |
| --- | --- | --- |
| Browser or Web Worker, analysis only | `@mermanjs/web-analysis` | Detection, validation, facts, and diagnostics without SVG, ASCII, or editor sessions |
| Browser application with several Merman workflows | `@mermanjs/web` | One complete browser artifact for rendering, analysis, ASCII, and editor APIs |
| Node.js, CI, or static-site automation | `merman-cli` child process | Supported browserless process boundary; JSON analysis is available |
| Generic editor or LSP client | `merman-lsp` | Language intelligence only; preview rendering remains separate |
| VS Code | Merman VS Code extension | Local LSP plus optional preview, export, diagnostics, and source actions |
| Rust application | `merman-analysis`, `merman-editor-core`, or `merman` | In-process typed APIs selected by capability |

The browser package group on the current branch may still be a source candidate. Consult the [browser package guide](../../platforms/web/README.md) before assuming that a candidate package is available on npm.

## Browser Analysis

Use `@mermanjs/web-analysis` for browser linting, diagram detection, and metadata workflows. It exports the same document-analysis API as the complete browser package without compiling SVG, ASCII, or editor sessions:

```ts
import { analyzeDocument, initMerman } from "@mermanjs/web-analysis";

await initMerman();

const result = analyzeDocument(
  markdownSource,
  { lint: { profile: "recommended" } },
  "file:///workspace/README.md",
);
```

`analyzeDocument()` supports standalone `.mmd`, Markdown, and MDX source modeling. Diagnostics retain host-document spans, related locations, help text, and fix edits. Use the complete `@mermanjs/web` package instead when the same browser realm also needs several of rendering, ASCII, analysis, and editor sessions.

Both packages require a browser main-thread or Web Worker realm. Do not load them from Node.js, SSR, or a Node-based lint runner through a compatibility shim.

## Node.js And CI

The supported Node.js and CI boundary is `merman-cli`. Invoke it as a child process and consume its machine-readable output:

```sh
merman-cli lint --format json diagram.mmd
merman-cli lint --markdown --format json README.md
```

The in-process `@mermanjs/node` workspace is a private transport candidate, not an admitted package. Do not depend on it until its installation, target, parity, lifecycle, and footprint gates pass. See the [CLI guide](../../crates/merman-cli/README.md) for installation and for a slim analysis-only build.

## Editors

Use [`merman-lsp`](../../crates/merman-lsp/README.md) for completion, hover, document symbols, navigation, rename, selection and folding ranges, semantic tokens, diagnostics, and fix-backed code actions. The LSP does not render previews; let the editor's built-in Markdown preview, another Mermaid preview extension, or `merman-cli` own rendering.

The [VS Code extension](../../tools/vscode-extension/README.md) combines the local LSP with optional preview and export. Disable only the overlapping surface when another tool owns it:

```json
{
  "merman.sourceActions.enabled": false,
  "merman.diagnostics.enabled": false
}
```

These settings leave the language server running. See [VS Code coexistence](vscode-coexistence.md) for common preview and diagnostics combinations.

## Lint Ownership

External tools can use Merman diagnostics as evidence without transferring all lint policy to Merman. Keep repository discovery, ignore rules, CI formatting and exit policy, Mermaid.js fallback, and non-Merman style rules in the integrating tool when those are part of its contract.

Merman-owned rules retain their `merman.*` IDs. Do not map unrelated markdownlint, remark, textlint, or project-specific rules into that namespace. The detailed [lint interop guide](lint-interop.md) describes rule ownership, adapter shape, spans, and fixes.

## Non-Goals

Merman does not provide a Mermaid.js-authoritative runtime fallback, replace every Mermaid lint ecosystem, require users to disable VS Code's built-in Mermaid Markdown preview, or treat a browser-only WASM package as a Node.js transport.
