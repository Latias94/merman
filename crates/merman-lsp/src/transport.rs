use futures::Sink;
use tokio::io::{AsyncRead, AsyncWrite};
use tower_lsp::jsonrpc::Response;
use tower_lsp::{Loopback, Server};

/// Maximum number of client requests the stdio transport may process concurrently.
///
/// Keep workspace-wide requests from monopolizing the handler loop while document epochs and
/// snapshot generations guard response freshness.
pub const LSP_HANDLER_CONCURRENCY: usize = 4;

/// Builds the stdio LSP transport with Merman's production concurrency policy.
pub fn stdio_server<I, O, L>(stdin: I, stdout: O, socket: L) -> Server<I, O, L>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite,
    L: Loopback,
    <L::ResponseSink as Sink<Response>>::Error: std::error::Error,
{
    Server::new(stdin, stdout, socket).concurrency_level(LSP_HANDLER_CONCURRENCY)
}
