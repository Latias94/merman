use crate::resources::InputResourceLimitExceeded;

/// A presentation configuration exceeded its construction budget.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresentationConfigError {
    #[error(transparent)]
    ResourceLimit(#[from] InputResourceLimitExceeded),
    #[error("presentation config construction size overflow")]
    SizeOverflow,
}
