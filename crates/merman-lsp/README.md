# merman-lsp

[![Crates.io](https://img.shields.io/crates/v/merman-lsp.svg)](https://crates.io/crates/merman-lsp) [![Documentation](https://docs.rs/merman-lsp/badge.svg)](https://docs.rs/merman-lsp) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Local Mermaid language intelligence for `.mmd`, `.mermaid`, Markdown, and MDX documents.

`merman-lsp` provides strict-parser-backed diagnostics, completion, hover, navigation, rename,
code actions, symbols, and folding. Syntax highlighting uses the canonical
`tree-sitter-mermaid` grammar and highlight query. It does not render previews; pair it with
[`merman-cli`](https://crates.io/crates/merman-cli), or use the
[Merman VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme)
for an integrated editor experience.

## Install The Stdio Server

The crate defaults to a protocol-neutral Rust library. Enable `stdio` when installing the bundled language-server executable:

```sh
cargo install merman-lsp --version 0.8.0-alpha.6 --locked \
  --no-default-features --features stdio
```

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

Strict language behavior comes from `merman-analysis` and `merman-editor-core`. A separate
document-owned Tree-sitter state provides tolerant syntax captures without waiting for an analysis
generation. Standalone Mermaid documents own one incremental tree; Markdown and MDX documents own
one tree per Mermaid fence with syntax-side host coordinate mapping. `MermanLspService` owns
synchronous message admission, while its private `LanguageSession` owns document and configuration
transactions, generation acquisition and caching, stale-result suppression, cancellation, client
effects, refresh coordination, and shutdown. The remaining LSP layer projects those results onto
the protocol.

Unconfigured sessions admit at most 4 MiB of source text and 256 Mermaid fences per Markdown or MDX document. Source-byte rejection discards the oversized text and requires a later full replacement. Fence-count rejection retains authoritative text, so ranged edits or a configuration increase can restore analysis without reopening the document. Both limits are exposed through the analysis settings and `merman/configSchema`.

## Language Features

- Diagnostics, including pull diagnostics and fix-backed quick fixes. When several requested diagnostics own the same shared document fix, the server materializes one equivalent workspace edit rather than repeating the action.
- Completion and completion-item resolution.
- Hover, document symbols, selection ranges, and folding ranges.
- Definition, references, prepare rename, and rename.
- Full-document, range, and delta semantic-token transport backed exclusively by Tree-sitter
  syntax captures.
- Merman extension requests for the rule catalog and configuration schema.

The server intentionally rejects workspace diagnostics until unopened workspace-file discovery has a defined owner. Formatting is not currently provided.

See the [capability matrix](https://github.com/Latias94/merman/blob/main/docs/lsp/CAPABILITIES.md) for family-level coverage.

## Embed The Protocol Library

Applications that already own a transport can depend on `merman-lsp` with default features disabled and call `MermanLanguageServer::service()` directly. The `stdio` feature adds only the bundled Tokio stdio transport and executable.

`MermanLanguageServer::service()` returns an ordered `MermanLspService` plus its client socket. Embedded transports must drive `MermanLspService` through `tower::Service<Request>` so each message is admitted in input order. The underlying `LanguageServer` is intentionally not exposed: calling it directly would bypass document and configuration ordering.

The session refactor removes the direct `MermanLanguageServer::rule_catalog()` and
`MermanLanguageServer::config_schema()` helpers. Embedded clients should send
`RULE_CATALOG_METHOD` and `CONFIG_SCHEMA_METHOD` through the ordered service. Rust callers that
only need the static payloads can use `RuleCatalogResponse::current()` and
`ConfigSchemaResponse::current()`. The configuration response is a protocol projection of
`merman_analysis::AnalysisConfigContract`; LSP contributes only its source and document-fence
defaults, not an independent accepted-shape definition.

```toml
[dependencies]
merman-lsp = { version = "=0.8.0-alpha.6", default-features = false }
```

The embedding boundary deliberately uses the same JSON-RPC and service types as `tower-lsp-server`. Declare those transport dependencies directly so Cargo resolves the traits and request types used by the host:

```toml
futures = "0.3.31"
tower = { version = "0.5.2", default-features = false, features = ["util"] }
tower-lsp-server = { version = "0.23.0", default-features = false, features = ["runtime-tokio"] }
```

Split the returned socket once, then drive both named halves. `MermanClientSocket` is an ownership token and deliberately does not implement `Stream` or `Sink`. Incoming client requests enter the ordered service; its response goes back to the client. Server-initiated requests and notifications leave through `MermanRequestStream`, and client responses to server-initiated requests must be sent through `MermanResponseSink`:

```rust
use futures::{SinkExt, StreamExt};
use merman_lsp::{
    MermanClientSocketError, MermanLanguageServer, MermanLspService, MermanRequestStream,
    MermanResponseSink,
};
use tower::{Service, ServiceExt};
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::ExitedError;

async fn handle_client_request(
    service: &mut MermanLspService,
    request: Request,
) -> Result<Option<Response>, ExitedError> {
    service.ready().await?.call(request).await
}

async fn next_server_request(requests: &mut MermanRequestStream) -> Option<Request> {
    requests.next().await
}

async fn handle_client_response(
    responses: &mut MermanResponseSink,
    response: Response,
) -> Result<(), MermanClientSocketError> {
    responses.send(response).await
}

fn create_session() -> (MermanLspService, MermanRequestStream, MermanResponseSink) {
    let (service, socket) = MermanLanguageServer::service();
    let (requests, responses) = socket.split();
    (service, requests, responses)
}
```

The host transport should poll these directions concurrently. It owns its queue and scheduling policy: reject encoded JSON bodies whose `Content-Length` exceeds `LSP_MAX_MESSAGE_BYTES`, bound retained queued and running JSON-body bytes according to `LSP_REQUEST_BYTE_BUDGET`, and use `LSP_ORDINARY_HANDLER_CONCURRENCY` as Merman's ordinary-work reference limit when appropriate. Call the ordered Tower service as each embedded message enters the host's bounded queue. There is no public `StdioService` hook or public control-lane concurrency contract; those were private bundled-transport details and have been removed.

The bundled stdio transport retains at most 96 admitted ordinary messages and polls four ordinary futures concurrently. Only a valid, ID-less `$/cancelRequest` or `exit` notification whose encoded JSON body is at most 4 KiB uses its private immediate-control path. An overloaded request receives JSON-RPC `-32099` (`Server overloaded`) only when the complete framed error response, including its `Content-Length` header, fits the bounded overload-output lane. An overloaded notification, an unreportable request overload, or exhaustion of that output lane terminates with `StdioTermination::InputOverloaded`. Once input integrity is lost, later unread cancellation or exit frames are not promised. If stdout fails while another stop reason is pending, `OutputClosed` wins; successful exit admission never hides output failure.

Dropping the service, the unsplit socket, or either split half terminates the whole shared session and cancels pending work exactly once. Request EOF, a successful response-sink close, and any response-sink error also terminate both directions. Closing the response half wakes a pending request poll, while closing the request half makes subsequent response operations fail immediately, so transport shutdown cannot wait forever. Do not leave either socket half undriven because diagnostics, logs, and refresh requests use them.

## Runtime And Contract Boundaries

LSP analysis uses deterministic runtime state. Initialization and workspace settings can provide `fixed_today` and `fixed_local_offset_minutes`. `fixed_today` uses the canonical `CivilDate` spelling: years `0000` through `9999` use `YYYY-MM-DD`, later years use `+YEAR-MM-DD`, and negative years use `-YEAR-MM-DD`. The server does not expose a native runtime selector or forward `system-*` Cargo features.

The private session cache consumes typed editor snapshots backed by `FenceTextIndex` and keeps reusable snapshot-only or complete analysis under one weighted budget; normal language requests do not serialize `AnalysisFactsPayload`. The separately exposed binding facts payload uses schema version `2`, which is independent from LSP document revisions and Mermaid diagram IDs such as `flowchart-v2`.

When a family parser cannot provide complete or recovered body facts, Merman does not guess body symbols, references, or rename targets. Source-start diagram headers and templates remain available from the static family catalog.

## Related Documentation

- [LSP architecture and lifecycle](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [VS Code extension](https://github.com/Latias94/merman/tree/main/tools/vscode-extension#readme)
- [Analysis crate](https://crates.io/crates/merman-analysis)
- [Editor-core crate](https://docs.rs/merman-editor-core)
