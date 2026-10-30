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
    /// Curated design treatment by family; absent families and unknown treatments are unreviewed.
    /// This does not imply mechanism support, readability, or target qualification.
    #[serde(default)]
    pub family_designs: Vec<ThemePresetFamilyDesignV1>,
    /// Representative colors from the recipe, for discovery swatches rather than rendered proof.
    #[serde(default)]
    pub preview_colors: Vec<String>,
    /// Curated application guidance. Recommendations require an explicit caller choice.
    #[serde(default)]
    pub application_notes: Vec<ThemePresetApplicationNoteV1>,
    /// Scoped catalog declarations; these are not execution receipts.
    pub qualified_cells: Vec<ThemePresetQualifiedCellV1>,
    /// SPDX expression for the preset recipe.
    pub license_expression: String,
    /// Attribution required when redistributing the preset, if any.
    pub required_attribution: Option<String>,
    /// Recipe representation returned by preset export.
    pub export_kind: String,
}

/// Curated visual design scope, independent of compilation and output qualification.
///
/// `base_only` means shared recipe styling, including necessary family adaptations, rather than
/// a dedicated design promised by this preset. `dedicated` denotes an intentional family design;
/// it does not certify every scene or output. Missing families and unknown treatments must be
/// displayed as unreviewed. IDs remain open and must survive older consumers unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetFamilyDesignV1 {
    /// Stable logical diagram-family ID, not a parser variant or render implementation ID.
    pub family_id: String,
    /// Open classification: `dedicated`, `base_only`, or `unreviewed`.
    pub treatment: String,
}

/// Human-readable guidance for a preset's family-specific design.
///
/// This is discovery metadata, not an executable requirement or a support certificate.
/// Preserve unknown IDs and show their message. Never change source/configuration merely
/// because a preset was selected; a host can offer the suggested settings as an explicit action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetApplicationNoteV1 {
    /// Open identifier used by hosts that localize this guidance.
    pub id: String,
    /// Logical diagram family to which the guidance applies.
    pub family_id: String,
    /// Readable fallback text, including conditions not expressible as configuration.
    pub message: String,
    /// A partial Mermaid configuration object; unrelated caller settings must be preserved.
    pub recommended_config: Option<serde_json::Map<String, serde_json::Value>>,
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
        let vectors: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/qualified-cells.json"
        ))
        .unwrap();
        let distinct_profiles = ["future-profile-portable", "host-dependent"]
            .map(|id| &vectors.iter().find(|vector| vector["id"] == id).unwrap()["cell"]);
        let groups = vectors
            .iter()
            .map(|vector| serde_json::json!([vector["cell"]]))
            .chain([serde_json::json!(distinct_profiles)]);
        for cells in groups {
            let json = serde_json::json!({
                "id": "future-preset", "display_name": "Future preset",
                "appearance": "future-appearance", "maturity": "future-maturity",
                "available": false, "availability_reason_ids": ["future-reason"],
                "qualified_cells": cells,
                "family_designs": [{"family_id": "future-family", "treatment": "future-design"}],
                "preview_colors": ["#123456"],
                "application_notes": [{
                    "id": "future-note", "family_id": "future-family", "message": "Future guidance",
                    "recommended_config": {"future": {"option": true}}
                }],
                "license_expression": "MIT OR Apache-2.0",
                "required_attribution": "Future attribution", "export_kind": "future-export"
            });
            let metadata: ThemePresetMetadataV1 = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(serde_json::to_value(metadata).unwrap(), json);
        }
    }

    #[test]
    fn older_catalog_entries_default_optional_discovery_guidance() {
        let mut json = serde_json::to_value(ThemePresetMetadataV1 {
            id: "old".into(),
            display_name: "Old".into(),
            appearance: "light".into(),
            maturity: "alpha".into(),
            available: true,
            availability_reason_ids: vec![],
            family_designs: vec![],
            preview_colors: vec![],
            application_notes: vec![],
            qualified_cells: vec![],
            license_expression: "MIT".into(),
            required_attribution: None,
            export_kind: "design".into(),
        })
        .unwrap();
        json.as_object_mut().unwrap().remove("preview_colors");
        json.as_object_mut().unwrap().remove("application_notes");
        let metadata: ThemePresetMetadataV1 = serde_json::from_value(json).unwrap();
        assert!(metadata.preview_colors.is_empty());
        assert!(metadata.application_notes.is_empty());
    }
}
