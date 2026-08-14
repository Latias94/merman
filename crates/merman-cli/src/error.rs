use crate::input::InputReadError;
use std::path::Path;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputRole {
    Primary,
    Auxiliary,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileOperation {
    Inspect,
    InspectIdentity,
    Canonicalize,
    VerifyPublication,
    #[cfg(feature = "analysis")]
    VerifySourceSnapshot,
    #[cfg(feature = "markdown")]
    ReadDirectory,
    #[cfg(feature = "markdown")]
    CreateDirectory,
    OpenAtomicStaging,
    WriteAtomicStaging,
    CommitAtomicReplacement,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl std::fmt::Display for FileOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Inspect => "inspect",
            Self::InspectIdentity => "inspect the file identity of",
            Self::Canonicalize => "resolve",
            Self::VerifyPublication => "verify the preflight identity of",
            #[cfg(feature = "analysis")]
            Self::VerifySourceSnapshot => "verify the acquired source snapshot of",
            #[cfg(feature = "markdown")]
            Self::ReadDirectory => "scan",
            #[cfg(feature = "markdown")]
            Self::CreateDirectory => "create the output directory",
            Self::OpenAtomicStaging => "open an atomic staging file for",
            Self::WriteAtomicStaging => "write the atomic staging file for",
            Self::CommitAtomicReplacement => "atomically replace",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorCategory {
    Success,
    Content,
    Usage,
    Operational,
}

#[cfg(feature = "ascii")]
#[derive(Debug)]
pub(crate) struct AsciiResourceError {
    details: merman::render::ResourceLimitExceeded,
    profile: merman::resources::ResourceProfile,
}

#[cfg(feature = "ascii")]
impl std::fmt::Display for AsciiResourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "ASCII resource limit `{}` exceeded during `{}`: actual {}, maximum {} (profile `{}`)",
            self.details.id,
            self.details.phase,
            self.details.actual,
            self.details.maximum,
            self.profile.id(),
        )?;
        if self.details.cause == merman::render::ResourceLimitCause::ArithmeticOverflow {
            write!(formatter, " (cause `{}`)", self.details.cause)?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Mermaid(#[from] merman::Error),
    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[error("{0}")]
    Render(#[from] merman::RenderError),
    #[cfg(feature = "ascii")]
    #[error("{0}")]
    Ascii(merman::ascii::AsciiDiagnostic),
    #[cfg(feature = "ascii")]
    #[error("{0}")]
    AsciiResource(AsciiResourceError),
    #[cfg(feature = "network-icons")]
    #[error("{0}")]
    Network(#[from] crate::network::NetworkError),
    #[cfg(any(test, feature = "svg", feature = "ascii"))]
    #[error("{0}")]
    Resource(#[from] crate::resources::ResourceLedgerError),
    #[cfg(feature = "markdown")]
    #[error("{0}")]
    Transaction(#[from] crate::transaction::TransactionError),
    #[error("stdout closed before output finished")]
    BrokenStdoutPipe,
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to serialize JSON output: {0}")]
    JsonOutput(serde_json::Error),
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    #[error("failed to {operation} {path:?}: {source}")]
    File {
        operation: FileOperation,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {stream}: {source}")]
    Stream {
        stream: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("No Mermaid diagram detected")]
    NoDiagram,
    #[error("{error}")]
    Input {
        role: InputRole,
        #[source]
        error: InputReadError,
    },
    #[error("no input was provided to `{command}`")]
    MissingInput { command: &'static str },
    #[error("{0}")]
    InvalidInput(String),
    #[cfg(feature = "icons")]
    #[error("internal error: {0}")]
    Internal(String),
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    #[error("{0}")]
    InvalidOutput(String),
    #[cfg(feature = "analysis")]
    #[error("refusing to overwrite concurrently modified source {path:?}: {reason}")]
    ConcurrentModification { path: PathBuf, reason: String },
    #[cfg(feature = "analysis")]
    #[error("invalid diagnostic fix plan: {0}")]
    InvalidFixPlan(String),
    #[cfg(feature = "markdown")]
    #[error("Markdown chart {index} at line {line}, column {column}: {source}")]
    MarkdownChart {
        index: u64,
        line: usize,
        column: usize,
        #[source]
        source: Box<CliError>,
    },
}

impl CliError {
    #[cfg(feature = "ascii")]
    pub(crate) fn ascii_resource(
        details: merman::render::ResourceLimitExceeded,
        profile: merman::resources::ResourceProfile,
    ) -> Self {
        Self::AsciiResource(AsciiResourceError { details, profile })
    }

    pub(crate) fn primary_input(error: InputReadError) -> Self {
        Self::Input {
            role: InputRole::Primary,
            error,
        }
    }

    pub(crate) fn auxiliary_input(error: InputReadError) -> Self {
        Self::Input {
            role: InputRole::Auxiliary,
            error,
        }
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    pub(crate) fn file(
        operation: FileOperation,
        path: impl AsRef<Path>,
        source: std::io::Error,
    ) -> Self {
        Self::File {
            operation,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub(crate) fn stream(stream: &'static str, source: std::io::Error) -> Self {
        if stream == "stdout" && source.kind() == std::io::ErrorKind::BrokenPipe {
            Self::BrokenStdoutPipe
        } else {
            Self::Stream { stream, source }
        }
    }

    pub(crate) fn json_output(error: serde_json::Error) -> Self {
        if error.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) {
            Self::BrokenStdoutPipe
        } else {
            Self::JsonOutput(error)
        }
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn markdown_chart(
        index: u64,
        location: crate::markdown::MarkdownFenceLocation,
        source: CliError,
    ) -> Self {
        Self::MarkdownChart {
            index,
            line: location.line,
            column: location.column,
            source: Box::new(source),
        }
    }

    pub(crate) fn exit_code(&self) -> ExitCode {
        match self.category() {
            ErrorCategory::Success => ExitCode::SUCCESS,
            ErrorCategory::Content => ExitCode::from(1),
            ErrorCategory::Usage => ExitCode::from(2),
            ErrorCategory::Operational => ExitCode::from(3),
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::InvalidInput(_) | Self::Json(_) | Self::MissingInput { .. } => {
                ErrorCategory::Usage
            }
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            Self::InvalidOutput(_) => ErrorCategory::Usage,
            Self::Io(_) | Self::JsonOutput(_) | Self::Stream { .. } => ErrorCategory::Operational,
            #[cfg(feature = "icons")]
            Self::Internal(_) => ErrorCategory::Operational,
            #[cfg(feature = "markdown")]
            Self::Transaction(_) => ErrorCategory::Operational,
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            Self::File { .. } => ErrorCategory::Operational,
            #[cfg(feature = "analysis")]
            Self::ConcurrentModification { .. } => ErrorCategory::Operational,
            #[cfg(feature = "analysis")]
            Self::InvalidFixPlan(_) => ErrorCategory::Content,
            #[cfg(feature = "markdown")]
            Self::MarkdownChart { source, .. } => source.category(),
            Self::Input { error, .. } if error.is_operational() => ErrorCategory::Operational,
            #[cfg(feature = "network-icons")]
            Self::Network(error) if error.is_operational() => ErrorCategory::Operational,
            Self::Input {
                role: InputRole::Primary,
                error: InputReadError::LimitExceeded { .. } | InputReadError::InvalidUtf8 { .. },
            } => ErrorCategory::Content,
            Self::Input { .. } => ErrorCategory::Usage,
            #[cfg(feature = "network-icons")]
            Self::Network(_) => ErrorCategory::Usage,
            Self::BrokenStdoutPipe => ErrorCategory::Success,
            Self::Mermaid(_) | Self::NoDiagram => ErrorCategory::Content,
            #[cfg(feature = "ascii")]
            Self::Ascii(_) | Self::AsciiResource(_) => ErrorCategory::Content,
            #[cfg(any(test, feature = "svg", feature = "ascii"))]
            Self::Resource(_) => ErrorCategory::Content,
            #[cfg(any(feature = "svg", feature = "ascii"))]
            Self::Render(_) => ErrorCategory::Content,
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

pub(crate) fn safe_path(path: impl AsRef<Path>) -> String {
    format!("{:?}", path.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_paths_escape_terminal_control_characters() {
        let rendered = safe_path(Path::new("line\nname\u{1b}[31m.svg"));
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains("\\n"));
        assert!(rendered.contains("\\u{1b}"));
    }
}
