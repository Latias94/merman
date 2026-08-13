use thiserror::Error;

pub type Result<T> = std::result::Result<T, AsciiError>;

#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AsciiError {
    #[error(transparent)]
    Cancelled(#[from] merman_core::OperationCancelled),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] merman_core::OperationResourceLimitExceeded),
    #[error("invalid ASCII render option `{field}`: {message}")]
    InvalidOption {
        field: &'static str,
        message: &'static str,
    },
    #[error("ASCII rendering does not support diagram type `{diagram_type}`")]
    UnsupportedDiagram { diagram_type: String },
    #[error("ASCII rendering does not support `{feature}` for `{diagram_type}` yet")]
    UnsupportedFeature {
        diagram_type: &'static str,
        feature: &'static str,
    },
}
