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

Applications that already own a transport can depend on `merman-lsp` with default features disabled and call `MermanLanguageServer::service()` directly. The `stdio` feature adds only the bundled Tokio stdio transport and executable.

`MermanLanguageServer::service()` returns an ordered `MermanLspService` plus its client socket. Embedded transports must drive `MermanLspService` through `tower::Service<Request>` so each message is admitted in input order. The underlying `LanguageServer` is intentionally not exposed: calling it directly would bypass document and configuration ordering.

<!-- BEGIN GENERATED RELEASE README LSP_LIBRARY_DEPENDENCY -->

```toml
[dependencies]
merman-lsp = { version = "=0.8.0-alpha.4", git = "https://github.com/Latias94/merman", default-features = false }
```

<!-- END GENERATED RELEASE README LSP_LIBRARY_DEPENDENCY -->

The embedding boundary deliberately uses the same JSON-RPC and service types as `tower-lsp-server`. Declare those transport dependencies directly so Cargo resolves the traits and request types used by the host:

```toml
futures = "0.3.31"
tower = { version = "0.5.2", default-features = false, features = ["util"] }
tower-lsp-server = { version = "0.23.0", default-features = false, features = ["runtime-tokio"] }
```

Drive both halves of the returned pair. Incoming client requests enter the ordered service; its response goes back to the client. Server-initiated requests and notifications leave through the socket, and client responses to server-initiated requests must be sent back into that socket:

```rust
use futures::{SinkExt, StreamExt};
use merman_lsp::{MermanClientSocket, MermanLanguageServer, MermanLspService};
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::ExitedError;

async fn handle_client_request(
    service: &mut MermanLspService,
    request: Request,
) -> Result<Option<Response>, ExitedError> {
    service.ready().await?.call(request).await
}

async fn next_server_request(socket: &mut MermanClientSocket) -> Option<Request> {
    socket.next().await
}

async fn handle_client_response(
    socket: &mut MermanClientSocket,
    response: Response,
) -> Result<(), ExitedError> {
    socket.send(response).await
}

fn create_session() -> (MermanLspService, MermanClientSocket) {
    MermanLanguageServer::service()
}
```

The host transport should poll these directions concurrently. It owns the request queue: reject encoded messages larger than `LSP_MAX_MESSAGE_BYTES`, retain no more than `LSP_REQUEST_BYTE_BUDGET` across queued and running messages, and poll at most `LSP_HANDLER_CONCURRENCY` handler futures concurrently. Call the service as each accepted message enters that bounded queue so a later `$/cancelRequest` can cancel a request that is still waiting for ordered admission. Dropping either half closes its associated pending work; do not leave the socket undriven because diagnostics, logs, and refresh requests use it.

## Runtime And Contract Boundaries

LSP analysis uses deterministic runtime state. Initialization and workspace settings can provide `fixed_today` and `fixed_local_offset_minutes`, but the server does not expose a native runtime selector or forward `system-*` Cargo features.

The document store consumes typed editor snapshots backed by `FenceTextIndex`; normal language requests do not serialize `AnalysisFactsPayload`. The separately exposed binding facts payload uses schema version `1`, which is independent from LSP document revisions and Mermaid diagram IDs such as `flowchart-v2`.

When a family parser cannot provide complete or recovered body facts, Merman does not guess body symbols, references, or rename targets. Source-start diagram headers and templates remain available from the static family catalog.

## Related Documentation

- [LSP architecture and lifecycle](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme)
- [Analysis crate](https://crates.io/crates/merman-analysis)
- [Editor-core crate](https://docs.rs/merman-editor-core)
