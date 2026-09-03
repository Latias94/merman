use thiserror::Error;

/// Errors produced while validating or decoding a DrawingList document.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DrawingListError {
    #[error("unsupported DrawingList version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("invalid DrawingList document: {0}")]
    InvalidDocument(String),
    #[error("DrawingList resource limit exceeded for {resource}: {actual} > {maximum}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("DrawingList JSON decode failed: {0}")]
    JsonDecode(String),
    #[error("DrawingList JSON encode failed: {0}")]
    JsonEncode(String),
}

impl DrawingListError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidDocument(message.into())
    }
}
