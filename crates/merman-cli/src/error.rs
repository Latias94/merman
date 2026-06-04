#[derive(Debug)]
pub(crate) enum CliError {
    Io(std::io::Error),
    Mermaid(merman::Error),
    Headless(merman::render::HeadlessError),
    #[cfg(feature = "ascii")]
    Ascii(merman::ascii::HeadlessAsciiError),
    Raster(merman::render::raster::RasterError),
    Json(serde_json::Error),
    NoDiagram,
    InvalidInput(String),
    InvalidOutput(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Mermaid(err) => write!(f, "{err}"),
            CliError::Headless(err) => write!(f, "{err}"),
            #[cfg(feature = "ascii")]
            CliError::Ascii(err) => write!(f, "{err}"),
            CliError::Raster(err) => write!(f, "{err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::NoDiagram => write!(f, "No Mermaid diagram detected"),
            CliError::InvalidInput(msg) => write!(f, "{msg}"),
            CliError::InvalidOutput(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<merman::Error> for CliError {
    fn from(value: merman::Error) -> Self {
        Self::Mermaid(value)
    }
}

impl From<merman::render::HeadlessError> for CliError {
    fn from(value: merman::render::HeadlessError) -> Self {
        Self::Headless(value)
    }
}

#[cfg(feature = "ascii")]
impl From<merman::ascii::HeadlessAsciiError> for CliError {
    fn from(value: merman::ascii::HeadlessAsciiError) -> Self {
        Self::Ascii(value)
    }
}

impl From<merman::render::raster::RasterError> for CliError {
    fn from(value: merman::render::raster::RasterError) -> Self {
        Self::Raster(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
