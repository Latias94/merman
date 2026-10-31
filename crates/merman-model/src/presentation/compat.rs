//! Borrowed values and local errors for Mermaid-compatible JSON projection.

use crate::MermaidConfig;

/// Values consumed by pure family JSON projectors.
///
/// Both source construction and independent output owners can borrow these values. No source
/// admission, initialization state, mutation journal, or ambient runtime is represented here.
#[derive(Debug, Clone, Copy)]
pub struct CompatJsonMetadata<'a> {
    pub diagram_type: &'a str,
    pub config: &'a MermaidConfig,
    pub authored_config: &'a MermaidConfig,
    pub title: Option<&'a str>,
    /// Explicit output identity where the family JSON shape contains one.
    pub diagram_id: Option<&'a str>,
}

impl<'a> From<&'a super::PresentationMetadata> for CompatJsonMetadata<'a> {
    fn from(metadata: &'a super::PresentationMetadata) -> Self {
        Self {
            diagram_type: &metadata.diagram_type,
            config: metadata.effective_config.as_config(),
            authored_config: &metadata.config,
            title: metadata.title.as_deref(),
            diagram_id: None,
        }
    }
}

/// A failure to project existing declared family data.
///
/// Source adapters map this local value back to their existing fallback diagnostic channel.
/// It carries no source span and does not classify a manually built model as a syntax failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Compatibility projection error ({diagram_type}): {message}")]
pub struct CompatProjectionError {
    diagram_type: String,
    message: String,
}

impl CompatProjectionError {
    pub fn invalid_model(diagram_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagram_type: diagram_type.into(),
            message: message.into(),
        }
    }

    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_parts(self) -> (String, String) {
        (self.diagram_type, self.message)
    }
}
