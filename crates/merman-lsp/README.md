# merman-lsp

[![Crates.io](https://img.shields.io/crates/v/merman-lsp.svg)](https://crates.io/crates/merman-lsp) [![Documentation](https://docs.rs/merman-lsp/badge.svg)](https://docs.rs/merman-lsp) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Local Mermaid language intelligence for `.mmd`, `.mermaid`, Markdown, and MDX documents.

`merman-lsp` provides parser-backed diagnostics, completion, hover, navigation, rename, code actions, symbols, folding, and semantic tokens. It does not render previews; pair it with [`merman-cli`](https://crates.io/crates/merman-cli), or use the [Merman VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme) for an integrated editor experience.

## Install The Stdio Server

The crate defaults to a protocol-neutral Rust library. Enable `stdio` when installing the bundled language-server executable:

<!-- BEGIN GENERATED RELEASE README LSP_INSTALL -->

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-lsp \
  --no-default-features --features stdio
```

<!-- END GENERATED RELEASE README LSP_INSTALL -->

Configure an editor or LSP client to launch:

```text
merman-lsp
```

The server communicates over standard input and output. Logs go to standard error, so protocol messages remain isolated.

For repository development:

```sh
cargo run -p merman-lsp --features stdio
```

## Supported Documents

- Standalone Mermaid files: `.mmd` and `.mermaid`.
- Mermaid code fences in Markdown, MDX, and Markdown-family documents.
- Multiple fences per document with source ranges mapped back to the host file.

Language behavior comes from `merman-analysis` and `merman-editor-core`; the LSP layer only owns document lifecycle, request cancellation, stale-result suppression, and protocol projection.

## Language Features

- Diagnostics, including pull diagnostics and fix-backed quick fixes.
- Completion and completion-item resolution.
- Hover, document symbols, selection ranges, and folding ranges.
- Definition, references, prepare rename, and rename.
- Full-document and range semantic tokens.
- Merman extension requests for the rule catalog and configuration schema.

The server intentionally rejects workspace diagnostics until unopened workspace-file discovery has a defined owner. Formatting is not currently provided.

See the [capability matrix](https://github.com/Latias94/merman/blob/main/docs/lsp/CAPABILITIES.md) for family-level coverage.

## Embed The Protocol Library

Applications that already own a transport can depend on `merman-lsp` with default features disabled and construct `MermanLanguageServer` directly. The `stdio` feature adds only the bundled Tokio stdio transport and executable.

<!-- BEGIN GENERATED RELEASE README LSP_LIBRARY_DEPENDENCY -->

```toml
[dependencies]
merman-lsp = { version = "=0.8.0-alpha.4", git = "https://github.com/Latias94/merman", default-features = false }
```

<!-- END GENERATED RELEASE README LSP_LIBRARY_DEPENDENCY -->

## Runtime And Contract Boundaries

LSP analysis uses deterministic runtime state. Initialization and workspace settings can provide `fixed_today` and `fixed_local_offset_minutes`, but the server does not expose a native runtime selector or forward `system-*` Cargo features.

The document store consumes typed editor snapshots backed by `FenceTextIndex`; normal language requests do not serialize `AnalysisFactsPayload`. The separately exposed binding facts payload uses schema version `1`, which is independent from LSP document revisions and Mermaid diagram IDs such as `flowchart-v2`.

When a family parser cannot provide complete or recovered body facts, Merman does not guess body symbols, references, or rename targets. Source-start diagram headers and templates remain available from the static family catalog.

## Related Documentation

- [LSP architecture and lifecycle](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme)
- [Analysis crate](https://crates.io/crates/merman-analysis)
- [Editor-core crate](https://docs.rs/merman-editor-core)
