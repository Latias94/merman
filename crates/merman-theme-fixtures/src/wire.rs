use crate::model::{
    AssetMediaType, ExpectedOutputTarget, ExpectedPortabilityGrade, ExpectedThemeCapability,
    FixtureEvidenceKind, MermaidStyleAdmission, MermaidStyleFamily, MermaidStyleOrigin,
    MermaidStylePlane, MermaidStyleProperty, ReferenceBlendMode, ReferenceDiagramFamily,
    ReferenceFontBinding, ReferenceGradientRepetition, ReferenceMechanismDisposition,
    ReferenceSemanticTarget, ReferenceTextTransform, ReferenceThemeFacet, ReferenceThemeMechanism,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManifestWire {
    pub(crate) schema_version: u32,
    pub(crate) sources: Vec<SourceRecordWire>,
    pub(crate) assets: Vec<AssetRecordWire>,
    pub(crate) fixtures: Vec<FixtureWire>,
    pub(crate) mechanism_translations: Vec<ReferenceTranslationWire>,
    pub(crate) themes: Vec<ReferenceThemeWire>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceEvidenceRecordWire {
    pub(crate) source_path: String,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceRecordWire {
    pub(crate) id: String,
    pub(crate) project: String,
    pub(crate) revision: String,
    pub(crate) upstream_license_path: String,
    pub(crate) license_path: String,
    pub(crate) license_sha256: String,
    pub(crate) snapshot_path: String,
    pub(crate) snapshot_sha256: String,
    pub(crate) evidence: Vec<SourceEvidenceRecordWire>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AssetRecordWire {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) media_type: AssetMediaType,
    pub(crate) canonical_family: String,
    pub(crate) family: String,
    pub(crate) weight: u16,
    pub(crate) style: String,
    pub(crate) unicode_range: String,
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) license_path: String,
    pub(crate) license_sha256: String,
    pub(crate) notice_path: String,
    pub(crate) notice_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FixtureWire {
    pub(crate) id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) expectation_path: String,
    pub(crate) expectation_sha256: String,
    pub(crate) theme_input_path: Option<String>,
    pub(crate) theme_input_sha256: Option<String>,
    pub(crate) evidence_source_ids: BTreeSet<String>,
    pub(crate) asset_ids: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FixtureExpectationWire {
    pub(crate) fixture_expectation_version: u32,
    pub(crate) fixture_id: String,
    pub(crate) evidence_kind: FixtureEvidenceKind,
    pub(crate) intent: String,
    #[serde(default)]
    pub(crate) reference_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    #[serde(default)]
    pub(crate) style_evidence_ids: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceThemeTokensWire {
    pub(crate) background: String,
    pub(crate) surface: String,
    pub(crate) primary: String,
    pub(crate) text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceFontStackWire {
    pub(crate) binding: ReferenceFontBinding,
    pub(crate) families: Vec<String>,
    pub(crate) asset_ids: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceTypographyInputWire {
    pub(crate) font_stack: Option<ReferenceFontStackWire>,
    pub(crate) letter_spacing_milli_em: Option<i16>,
    pub(crate) text_transform: Option<ReferenceTextTransform>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceGradientStopWire {
    pub(crate) offset_percent: u8,
    pub(crate) color: String,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ReferenceCanvasLayerWire {
    LinearGradient {
        angle_degrees: u16,
        repetition: ReferenceGradientRepetition,
        tile_width_px: Option<u16>,
        tile_height_px: Option<u16>,
        stops: Vec<ReferenceGradientStopWire>,
    },
    RadialGradient {
        center_x_percent: u8,
        center_y_percent: u8,
        repetition: ReferenceGradientRepetition,
        tile_width_px: Option<u16>,
        tile_height_px: Option<u16>,
        stops: Vec<ReferenceGradientStopWire>,
    },
    Solid {
        color: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceBorderInputWire {
    pub(crate) color: String,
    pub(crate) width_px: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceShadowInputWire {
    pub(crate) offset_x_px: i16,
    pub(crate) offset_y_px: i16,
    pub(crate) blur_px: u16,
    pub(crate) spread_px: u16,
    pub(crate) color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceNodeStyleInputWire {
    pub(crate) border: Option<ReferenceBorderInputWire>,
    pub(crate) dash_pattern: Vec<u16>,
    pub(crate) corner_radius_px: Option<u16>,
    pub(crate) shadow: Option<ReferenceShadowInputWire>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceSemanticStylePatchWire {
    pub(crate) fill: Option<String>,
    pub(crate) border: Option<ReferenceBorderInputWire>,
    pub(crate) corner_radius_px: Option<u16>,
    pub(crate) shadow: Option<ReferenceShadowInputWire>,
    pub(crate) text_color: Option<String>,
    pub(crate) font_weight: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ReferenceSemanticRuleWire {
    HasDescendant {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        descendant: ReferenceSemanticTarget,
        apply: ReferenceSemanticStylePatchWire,
    },
    NotClass {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        class_name: String,
        apply: ReferenceSemanticStylePatchWire,
    },
    OrdinalPalette {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        colors: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ReferenceSvgEffectWire {
    TurbulenceDisplacement {
        base_frequency_milli: u16,
        octaves: u8,
        scale: u16,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceThemeInputWire {
    pub(crate) theme_input_version: u32,
    pub(crate) fixture_id: String,
    pub(crate) tokens: Option<ReferenceThemeTokensWire>,
    pub(crate) typography: Option<ReferenceTypographyInputWire>,
    pub(crate) canvas: Vec<ReferenceCanvasLayerWire>,
    pub(crate) blend_mode: Option<ReferenceBlendMode>,
    pub(crate) node_style: Option<ReferenceNodeStyleInputWire>,
    pub(crate) semantic_rules: Vec<ReferenceSemanticRuleWire>,
    pub(crate) svg_effects: Vec<ReferenceSvgEffectWire>,
    pub(crate) residual_mechanisms: BTreeSet<ReferenceThemeMechanism>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceTranslationWire {
    pub(crate) source: ReferenceThemeMechanism,
    pub(crate) disposition: ReferenceMechanismDisposition,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
    #[serde(default)]
    pub(crate) target_overrides: BTreeMap<ExpectedOutputTarget, TargetTranslationWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TargetTranslationWire {
    pub(crate) disposition: ReferenceMechanismDisposition,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ResidualWire {
    pub(crate) id: String,
    pub(crate) source_mechanism: ReferenceThemeMechanism,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TargetPolicyEntryWire {
    pub(crate) grade: ExpectedPortabilityGrade,
    #[serde(default)]
    pub(crate) residuals: Vec<ResidualWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TargetPolicyWire {
    pub(crate) default: TargetPolicyEntryWire,
    #[serde(default)]
    pub(crate) overrides: BTreeMap<ExpectedOutputTarget, TargetPolicyEntryWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReferenceThemeWire {
    pub(crate) id: String,
    pub(crate) reference_name: String,
    pub(crate) source_id: String,
    pub(crate) fixture_ids: BTreeSet<String>,
    pub(crate) target_policy: TargetPolicyWire,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceSnapshotHeader {
    pub(crate) snapshot_version: u32,
    pub(crate) source_id: String,
    pub(crate) revision: String,
    pub(crate) evidence_files: Vec<SnapshotEvidenceFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModernMermaidSnapshot {
    pub(crate) snapshot_version: u32,
    pub(crate) source_id: String,
    pub(crate) revision: String,
    pub(crate) evidence_files: Vec<SnapshotEvidenceFile>,
    pub(crate) themes: Vec<SnapshotThemeRecord>,
    pub(crate) notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SnapshotEvidenceFile {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SnapshotThemeRecord {
    pub(crate) reference_name: String,
    pub(crate) source_line: u32,
    pub(crate) canvas_layer_count: u8,
    pub(crate) mechanisms: BTreeSet<ReferenceThemeMechanism>,
    #[serde(default)]
    pub(crate) facets: BTreeSet<ReferenceThemeFacet>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StylePrecedenceEvidenceWire {
    pub(crate) id: String,
    pub(crate) family: MermaidStyleFamily,
    pub(crate) origin: MermaidStyleOrigin,
    pub(crate) property: MermaidStyleProperty,
    pub(crate) plane: MermaidStylePlane,
    pub(crate) admission: MermaidStyleAdmission,
    pub(crate) rank: Option<u16>,
    pub(crate) encounter_order: bool,
    pub(crate) evidence: Vec<String>,
    pub(crate) note: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MermaidStylePrecedenceSnapshot {
    pub(crate) snapshot_version: u32,
    pub(crate) source_id: String,
    pub(crate) revision: String,
    pub(crate) evidence_files: Vec<SnapshotEvidenceFile>,
    pub(crate) entries: Vec<StylePrecedenceEvidenceWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExcalidrawFontSnapshot {
    pub(crate) asset_id: String,
    pub(crate) source_path: String,
    pub(crate) sha256: String,
    pub(crate) family: String,
    pub(crate) runtime_family: String,
    pub(crate) unicode_range: String,
    pub(crate) license: String,
    #[serde(default)]
    pub(crate) fixture_codepoints: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExcalidrawFontProvenanceSnapshot {
    pub(crate) snapshot_version: u32,
    pub(crate) source_id: String,
    pub(crate) revision: String,
    pub(crate) evidence_files: Vec<SnapshotEvidenceFile>,
    pub(crate) fonts: Vec<ExcalidrawFontSnapshot>,
    pub(crate) license: String,
}
