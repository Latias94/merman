//! Frozen metadata used to interpret existing presentation data.

use super::PresentationConfig;
use crate::MermaidConfig;

/// Resolved values and declared output ownership, independent of source admission.
///
/// A checked presentation diagram owns this metadata immutably. Constructing or cloning these
/// values does not create a core parsed result or authenticate the producer's history.
#[derive(Debug, Clone)]
pub struct PresentationMetadata {
    pub diagram_type: String,
    pub config: MermaidConfig,
    pub effective_config: PresentationConfig,
    pub title: Option<String>,
}
