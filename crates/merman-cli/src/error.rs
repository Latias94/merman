use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Mermaid(#[from] merman::Error),
    #[cfg(feature = "svg")]
    #[error("{0}")]
    Headless(#[from] merman::svg::HeadlessError),
    #[cfg(feature = "ascii")]
    #[error("{0}")]
    Ascii(#[from] merman::ascii::HeadlessAsciiError),
    #[error("stdout closed before output finished")]
    BrokenStdoutPipe,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[error("{0}")]
    Export(#[from] merman::svg::export::ExportError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No Mermaid diagram detected")]
    NoDiagram,
    #[error("no input was provided to `{command}`")]
    MissingInput { command: &'static str },
    #[error("{0}")]
    InvalidInput(String),
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    #[error("{0}")]
    InvalidOutput(String),
}

impl CliError {
    pub(crate) fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidInput(_) | Self::Json(_) => ExitCode::from(2),
            Self::MissingInput { .. } => ExitCode::from(2),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            Self::InvalidOutput(_) => ExitCode::from(2),
            Self::Io(_) => ExitCode::from(3),
            Self::BrokenStdoutPipe => ExitCode::SUCCESS,
            Self::Mermaid(_) | Self::NoDiagram => ExitCode::from(1),
            #[cfg(feature = "svg")]
            Self::Headless(_) => ExitCode::from(1),
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            Self::Export(_) => ExitCode::from(1),
            #[cfg(feature = "ascii")]
            Self::Ascii(_) => ExitCode::from(1),
        }
    }

    pub(crate) fn is_broken_stdout_pipe(&self) -> bool {
        matches!(self, Self::BrokenStdoutPipe)
    }

    pub(crate) fn missing_input_command(&self) -> Option<&'static str> {
        match self {
            Self::MissingInput { command } => Some(command),
            _ => None,
        }
    }
}
