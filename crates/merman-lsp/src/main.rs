use merman_lsp::{MermanLanguageServer, stdio_server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = MermanLanguageServer::service();
    stdio_server(stdin, stdout, socket).serve(service).await;
}
