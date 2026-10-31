use std::sync::Arc;

use crate::resources::InputResourceLimitExceeded;

use super::ThemeCompatibilityFieldKind;

/// Invalid declared presentation configuration or a construction budget rejection.
///
/// These failures validate data relationships, not the history or authority of a producer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresentationConfigError {
    #[error(transparent)]
    ResourceLimit(#[from] InputResourceLimitExceeded),
    #[error("presentation config construction size overflow")]
    SizeOverflow,
    #[error("theme compatibility residual fields require a recipe")]
    MissingThemeRecipe,
    #[error("family paint inputs require a theme compatibility recipe")]
    MissingFamilyPaintRecipe,
    #[error("invalid {kind:?} theme compatibility field identity `{field}`")]
    InvalidFieldIdentity {
        field: Arc<str>,
        kind: ThemeCompatibilityFieldKind,
    },
    #[error("theme compatibility field `{field}` has no surviving assignment paths")]
    EmptyFieldPaths { field: Arc<str> },
    #[error("unsupported assignment path `{path}` for theme compatibility field `{field}`")]
    InvalidAssignmentPath { field: Arc<str>, path: Arc<str> },
    #[error("duplicate assignment path `{path}` for theme compatibility field `{field}`")]
    DuplicateAssignmentPath { field: Arc<str>, path: Arc<str> },
    #[error("duplicate theme compatibility field `{field}`")]
    DuplicateThemeField { field: Arc<str> },
    #[error("theme compatibility field `{field}` is absent from the normalized recipe")]
    FieldAbsentFromRecipe { field: Arc<str> },
    #[error("surviving theme compatibility assignment `{path}` has no configuration value")]
    MissingAssignmentValue { path: Arc<str> },
}
