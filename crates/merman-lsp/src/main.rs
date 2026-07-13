use merman_lsp::{MermanLanguageServer, StdioTermination, serve_stdio};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = MermanLanguageServer::service_with_refresh();
    match serve_stdio(stdin, stdout, socket, service).await {
        StdioTermination::ExitWithoutShutdown => ExitCode::FAILURE,
        StdioTermination::InputClosed | StdioTermination::ExitAfterShutdown => ExitCode::SUCCESS,
    }
}
