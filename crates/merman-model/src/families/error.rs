#[cfg(feature = "compat-json")]
pub mod compat;

use serde::{Deserialize, Serialize};

/// Typed semantic model for Merman's built-in suppressed-error diagram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDiagramModel {
    #[serde(rename = "type")]
    pub diagram_type: String,
    #[serde(
        default,
        rename = "errorMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub error_message: Option<String>,
}

impl ErrorDiagramModel {
    pub fn new(diagram_type: impl Into<String>) -> Self {
        Self {
            diagram_type: diagram_type.into(),
            error_message: None,
        }
    }
}
