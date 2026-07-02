use merman_lsp::MermanLanguageServer;
use tower_lsp::Server;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = MermanLanguageServer::service();
    Server::new(stdin, stdout, socket).serve(service).await;
}
