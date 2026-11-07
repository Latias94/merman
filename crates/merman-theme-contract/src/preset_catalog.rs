use serde::{Deserialize, Serialize};

use crate::ThemeAppearance;

/// Schema version of the artifact-owned preset discovery catalog.
pub const THEME_PRESET_CATALOG_SCHEMA_VERSION_V1: u32 = 1;

/// Discovery metadata for one canonical built-in design and its concrete appearances.
///
/// Availability admits the complete document under the caller's compiler policy. Every declared
/// appearance shares this gate; a passing branch cannot make a rejected pair available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetMetadataV1 {
    /// Stable canonical design identifier.
    pub id: String,
    /// Human-readable design name, without an appearance suffix.
    pub display_name: String,
    /// Declared concrete appearances, in presentation order.
    pub supported_appearances: Vec<ThemeAppearance>,
    /// Deterministic default used when a caller omits an appearance.
    pub default_appearance: ThemeAppearance,
    /// Discovery details for each supported appearance.
    pub appearances: Vec<ThemePresetAppearanceMetadataV1>,
    /// Catalog maturity, independently of availability and qualification.
    pub maturity: String,
    /// Whether the effective compiler admits the complete document.
    pub available: bool,
    /// Stable reasons why the complete document is unavailable.
    pub availability_reason_ids: Vec<String>,
    /// SPDX expression for the complete recipe.
    pub license_expression: String,
    /// Attribution required when redistributing the recipe, if any.
    pub required_attribution: Option<String>,
    /// Recipe representation returned by preset export.
    pub export_kind: String,
}

/// Appearance-specific discovery facts for a canonical preset.
///
/// Compilation availability is subject to the same complete-document gate as its parent. Design
/// scope, swatches, guidance, and qualification remain distinct from that admission result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetAppearanceMetadataV1 {
    /// Concrete appearance selected by this entry.
    pub appearance: ThemeAppearance,
    /// Exact preset ID for selecting or exporting only this concrete appearance.
    pub concrete_preset_id: String,
    /// Whether the complete document admits this appearance.
    pub available: bool,
    /// Complete-document admission reasons, never a branch-only success result.
    pub availability_reason_ids: Vec<String>,
    /// Curated family design scope, independently of compilation and output qualification.
    pub family_designs: Vec<ThemePresetFamilyDesignV1>,
    /// Representative recipe colors for this appearance.
    pub preview_colors: Vec<String>,
    /// Curated application guidance for this appearance.
    pub application_notes: Vec<ThemePresetApplicationNoteV1>,
    /// Scoped declarations for this appearance; these are not execution receipts.
    pub qualified_cells: Vec<ThemePresetQualifiedCellV1>,
}

/// A presentation group for literal themes from the pinned Mermaid registry.
///
/// Grouping never changes a source/configuration ID. Independent upstream themes, including
/// Default and Dark, remain separate designs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeMermaidDesignMetadataV1 {
    /// Canonical presentation ID, using the default appearance's literal Mermaid ID.
    pub id: String,
    /// Human-readable design name.
    pub display_name: String,
    /// Deterministic default appearance of this presentation group.
    pub default_appearance: ThemeAppearance,
    /// Supported appearances and their exact upstream IDs.
    pub appearances: Vec<ThemeMermaidAppearanceMetadataV1>,
}

/// An original Mermaid theme ID selected for one declared appearance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeMermaidAppearanceMetadataV1 {
    /// Concrete appearance of the literal upstream theme.
    pub appearance: ThemeAppearance,
    /// Unchanged Mermaid source/configuration identifier.
    pub theme_id: String,
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
    fn grouped_metadata_round_trips_open_discovery_values_and_appearance_local_guidance() {
        let json = serde_json::json!({
            "id": "future-design",
            "display_name": "Future design",
            "supported_appearances": ["light", "dark"],
            "default_appearance": "dark",
            "appearances": [{
                "appearance": "light",
                "concrete_preset_id": "future-design-day",
                "available": false,
                "availability_reason_ids": ["future-policy-reason"],
                "family_designs": [{"family_id": "future-family", "treatment": "future-design"}],
                "preview_colors": ["#fefefe", "#123456"],
                "application_notes": [{
                    "id": "future-light-note",
                    "family_id": "future-family",
                    "message": "Use the light appearance.",
                    "recommended_config": {"future": {"option": true}}
                }],
                "qualified_cells": [{
                    "family_id": "future-family",
                    "output_id": "future-output",
                    "profile_id": "future-profile",
                    "admission_status": "future-admission"
                }]
            }, {
                "appearance": "dark",
                "concrete_preset_id": "future-design-night",
                "available": false,
                "availability_reason_ids": ["future-policy-reason"],
                "family_designs": [],
                "preview_colors": ["#121212", "#abcdef"],
                "application_notes": [],
                "qualified_cells": []
            }],
            "maturity": "future-maturity",
            "available": false,
            "availability_reason_ids": ["future-policy-reason"],
            "license_expression": "MIT OR Apache-2.0",
            "required_attribution": "Future attribution",
            "export_kind": "future-export"
        });
        let metadata: ThemePresetMetadataV1 = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(metadata.default_appearance, ThemeAppearance::Dark);
        assert_ne!(
            metadata.appearances[0].preview_colors,
            metadata.appearances[1].preview_colors
        );
        assert_eq!(serde_json::to_value(metadata).unwrap(), json);
    }

    #[test]
    fn qualification_profiles_remain_open_and_distinct_within_an_appearance() {
        let vectors: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../merman-theme-authoring-fixtures/fixtures/authoring-v1/qualified-cells.json"
        ))
        .unwrap();
        let cells = vectors
            .iter()
            .map(|vector| vector["cell"].clone())
            .collect::<Vec<_>>();
        let json = serde_json::json!({
            "appearance": "light",
            "concrete_preset_id": "future-concrete",
            "available": true,
            "availability_reason_ids": [],
            "family_designs": [],
            "preview_colors": [],
            "application_notes": [],
            "qualified_cells": cells
        });
        let metadata: ThemePresetAppearanceMetadataV1 =
            serde_json::from_value(json.clone()).unwrap();
        assert_eq!(metadata.qualified_cells.len(), vectors.len());
        assert_eq!(serde_json::to_value(metadata).unwrap(), json);
    }

    #[test]
    fn official_design_metadata_round_trips_literal_theme_ids() {
        let json = serde_json::json!({
            "id": "future-native",
            "display_name": "Future native",
            "default_appearance": "light",
            "appearances": [
                {"appearance": "light", "theme_id": "future-native"},
                {"appearance": "dark", "theme_id": "future-native-night"}
            ]
        });
        let metadata: ThemeMermaidDesignMetadataV1 = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(metadata).unwrap(), json);
    }
}
