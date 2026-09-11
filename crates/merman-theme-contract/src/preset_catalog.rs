use serde::{Deserialize, Serialize};

/// Schema version of the artifact-owned preset discovery catalog.
pub const THEME_PRESET_CATALOG_SCHEMA_VERSION_V1: u32 = 1;

/// Version 1 discovery metadata for one built-in preset under an effective compiler policy.
///
/// IDs and classifications are open strings. Availability describes recipe compilation only;
/// it does not grant qualification or imply support for a render output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetMetadataV1 {
    /// Stable preset identifier.
    pub id: String,
    /// Human-readable preset name.
    pub display_name: String,
    /// Appearance classification, such as `light` or `dark`.
    pub appearance: String,
    /// Catalog maturity, independently of availability and qualification.
    pub maturity: String,
    /// Whether the effective compiler can compile this preset.
    pub available: bool,
    /// Stable reasons why recipe compilation is unavailable.
    pub availability_reason_ids: Vec<String>,
    /// Scoped catalog declarations; these are not execution receipts.
    pub qualified_cells: Vec<ThemePresetQualifiedCellV1>,
    /// SPDX expression for the preset recipe.
    pub license_expression: String,
    /// Attribution required when redistributing the preset, if any.
    pub required_attribution: Option<String>,
    /// Recipe representation returned by preset export.
    pub export_kind: String,
}

/// Version 1 wire projection of one scoped preset qualification declaration.
///
/// Unknown profile and admission IDs must be preserved, never interpreted as portable support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetQualifiedCellV1 {
    /// Stable diagram-family identifier.
    pub family_id: String,
    /// Stable render-output identifier.
    pub output_id: String,
    /// Qualification scenario and its required resource conditions.
    pub profile_id: String,
    /// Declared target admission classification.
    pub admission_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_metadata_round_trips_open_identifiers_and_host_conditions() {
        let json = serde_json::json!({
            "id": "future-preset", "display_name": "Future preset",
            "appearance": "future-appearance", "maturity": "future-maturity",
            "available": false, "availability_reason_ids": ["future-reason"],
            "qualified_cells": [
                {"family_id": "state", "output_id": "png",
                 "profile_id": "native-state-system-fonts-v1",
                 "admission_status": "host_dependent"},
                {"family_id": "future-family", "output_id": "future-output",
                 "profile_id": "future-profile", "admission_status": "future-admission"}
            ],
            "license_expression": "MIT OR Apache-2.0",
            "required_attribution": "Future attribution", "export_kind": "future-export"
        });
        let metadata: ThemePresetMetadataV1 = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(metadata).unwrap(), json);
    }
}
