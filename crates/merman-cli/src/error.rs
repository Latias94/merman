use std::process::ExitCode;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Mermaid(#[from] merman::Error),
    #[error("{0}")]
    Headless(#[from] merman::render::HeadlessError),
    #[cfg(feature = "ascii")]
    #[error("{0}")]
    Ascii(#[from] merman::ascii::HeadlessAsciiError),
    #[error("{0}")]
    Raster(#[from] merman::render::raster::RasterError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("No Mermaid diagram detected")]
    NoDiagram,
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    InvalidOutput(String),
}

impl CliError {
    pub(crate) fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidInput(_) | Self::InvalidOutput(_) | Self::Json(_) => ExitCode::from(2),
            Self::Io(_) => ExitCode::from(3),
            Self::Mermaid(_) | Self::Headless(_) | Self::Raster(_) | Self::NoDiagram => {
                ExitCode::from(1)
            }
            #[cfg(feature = "ascii")]
            Self::Ascii(_) => ExitCode::from(1),
        }
    }

    pub(crate) fn is_broken_stdout_pipe(&self) -> bool {
        matches!(self, Self::Io(err) if err.kind() == std::io::ErrorKind::BrokenPipe)
    }
}
