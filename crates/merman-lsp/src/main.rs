use merman_lsp::MermanLanguageServer;
use tower_lsp::Server;

const LSP_HANDLER_CONCURRENCY: usize = 4;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = MermanLanguageServer::service();
    Server::new(stdin, stdout, socket)
        // Keep workspace-wide requests from monopolizing the handler loop while
        // document epochs and snapshot generations guard response freshness.
        .concurrency_level(LSP_HANDLER_CONCURRENCY)
        .serve(service)
        .await;
}
