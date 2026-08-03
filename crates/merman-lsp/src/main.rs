use merman_lsp::{MermanLanguageServer, StdioTermination, serve_stdio};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = MermanLanguageServer::service();
    termination_exit_code(serve_stdio(stdin, stdout, socket, service).await)
}

fn termination_exit_code(termination: StdioTermination) -> ExitCode {
    match termination {
        StdioTermination::ExitWithoutShutdown
        | StdioTermination::InputOverloaded
        | StdioTermination::OutputClosed => ExitCode::FAILURE,
        StdioTermination::InputClosed | StdioTermination::ExitAfterShutdown => ExitCode::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_overload_maps_to_process_failure() {
        assert_eq!(
            termination_exit_code(StdioTermination::InputOverloaded),
            ExitCode::FAILURE
        );
    }
}
