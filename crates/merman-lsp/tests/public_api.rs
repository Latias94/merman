#[cfg(feature = "stdio")]
use merman_lsp::{
    LSP_MAX_MESSAGE_BYTES, LSP_ORDINARY_HANDLER_CONCURRENCY, LSP_REQUEST_BYTE_BUDGET,
    MermanLanguageServer, StdioServer, StdioTermination, serve_stdio, stdio_server,
};

#[cfg(feature = "stdio")]
fn classify_termination(termination: StdioTermination) -> &'static str {
    match termination {
        StdioTermination::InputClosed => "input-closed",
        StdioTermination::OutputClosed => "output-closed",
        StdioTermination::ExitAfterShutdown => "exit-after-shutdown",
        StdioTermination::ExitWithoutShutdown => "exit-without-shutdown",
        StdioTermination::InputOverloaded => "input-overloaded",
    }
}

#[cfg(feature = "stdio")]
#[test]
fn stdio_public_contract_exposes_only_the_supported_embedding_surface() {
    let (_service, socket) = MermanLanguageServer::service();
    let server: StdioServer<_, _, _> = stdio_server(tokio::io::empty(), tokio::io::sink(), socket);
    let _server = server.ordinary_concurrency_level(LSP_ORDINARY_HANDLER_CONCURRENCY);
    let _serve = serve_stdio::<tokio::io::Empty, tokio::io::Sink, merman_lsp::MermanClientSocket>;

    assert!(LSP_ORDINARY_HANDLER_CONCURRENCY > 0);
    assert!(LSP_REQUEST_BYTE_BUDGET > 0);
    assert!(LSP_MAX_MESSAGE_BYTES > 0);
    assert_eq!(
        classify_termination(StdioTermination::InputOverloaded),
        "input-overloaded"
    );
}
