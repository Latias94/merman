use serde::{Deserialize, Serialize};

use crate::{DiagramThemeSpecWireV1, ThemeDefinitionV1};

/// Closed version 1 export envelope for one built-in preset recipe.
///
/// The tag is authoritative: callers never infer the payload kind from missing fields, and each
/// variant carries exactly one self-contained editable recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetExportV1 {
    /// A lossless compact authoring definition.
    Definition {
        /// The complete authoring definition for the preset.
        definition: ThemeDefinitionV1,
    },
    /// A lossless complete editable theme specification.
    CompleteSpec {
        /// The complete specification for the preset.
        complete_spec: DiagramThemeSpecWireV1,
    },
}

impl PresetExportV1 {
    /// Returns the stable wire tag for this export variant.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Definition { .. } => "definition",
            Self::CompleteSpec { .. } => "complete_spec",
        }
    }
}
