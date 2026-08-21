//! Portable, validated diagram-theme inputs and host admission policy.
//!
//! Per-mechanism root evidence is renderer-owned and is intentionally not part of this public
//! recipe surface:
//!
//! ```compile_fail
//! use merman_render::diagram_theme::RootThemeReport;
//! ```
//!
//! The lowering implementation is renderer-internal. Callers enter through the bounded
//! `materialize_theme` operations instead of constructing a materializer or bypassing admission:
//!
//! ```compile_fail
//! use merman_render::diagram_theme::ThemeMaterializer;
//! ```

mod admission;
mod application;
mod assets;
mod canonical;
mod canvas;
mod compiler;
mod definition_admission;
mod effects;
mod family_mechanism_matrix;
mod family_program;
mod legacy_family_theme_bridge;
mod legacy_projection_retirement;
mod materializer;
mod mechanisms;
mod mermaid_compatibility;
mod presets;
mod resolved;
mod resources;
mod semantic;
mod source_styles;
mod spec;
mod support;
mod typography;
mod wire_decode;

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub use merman_core::{MermaidThemeId, MermaidThemeIdParseError};
pub use merman_theme_contract::{
    DiagramThemeSpecWireV1, MaterializedThemeWireV1, THEME_SUPPORT_SCHEMA_VERSION_V2,
    ThemeCapabilityDescriptorV1, ThemeCapabilityDescriptorV2, ThemeDefinitionV1,
    ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1, ThemeRuleFacetV1,
    ThemeSupportBaseTypographyPropertyV2, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportQueryV2, ThemeSupportStateV1, ThemeSupportSubjectV2,
};

pub(crate) use admission::ResolvedThemeAdmission;
pub use admission::{
    FontEmbeddingRequirement, FontSource, FontSourcePolicy, HostMeasurementFallback,
    HostMeasurementFallbackPolicy, TextLayoutCapability, ThemeAdmissionError, ThemeAdmissionPolicy,
    ThemeCapability, ThemeHostAdmissionReport, ThemePortabilityRequirement, ThemeRequirements,
    TrustedThemeLane, TrustedThemeLanes,
};
#[cfg(test)]
pub(crate) use application::RootThemeEvaluation;
pub use application::RootThemeVerification;
pub(crate) use application::{
    FamilyThemeMechanismKey, RootThemeApplication, RootThemeMechanismKey, RootThemePlan,
    RootThemeReport, RootThemeResidualReason,
};
pub use assets::{
    FontAsset, FontAssetFingerprint, FontAssetIdError, FontAssetSpec, FontCatalog,
    FontCatalogError, FontCatalogFingerprint, FontCatalogSpec, FontContainer,
    FontEmbeddingPermissions, FontFaceMetadata, FontFamilyAlias, FontStyle, GenericFontFamily,
};
pub use canvas::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, GradientStop, InsetsPx, LinearGradient,
    PatternKind, PatternSpec, RadialGradient, ThemeColorValue, ThemeLength,
};
pub use compiler::{DiagramThemeCompiler, ThemeCompileError};
pub use definition_admission::{
    ThemeDefinitionCompileError, compile_theme_definition, compile_theme_definition_json,
    materialize_theme, materialize_theme_json, materialize_theme_json_with_resource_policy,
    materialize_theme_with_resource_policy,
};
pub use effects::{DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive};
#[cfg(feature = "internal-theme-acceptance")]
pub(crate) use family_mechanism_matrix::legacy_replacing_typed_routes;
pub(crate) use family_mechanism_matrix::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape,
};
#[cfg(feature = "internal-theme-acceptance")]
pub(crate) use legacy_family_theme_bridge::legacy_projection_retirement_receipts;
#[cfg(feature = "internal-theme-acceptance")]
#[doc(hidden)]
pub use legacy_projection_retirement::{
    ThemeLegacyProjectionKey, ThemeLegacyProjectionRetirementDescriptor,
    ThemeLegacyProjectionRetirementInventoryError, ThemeLegacyProjectionRetirementReceipt,
    ThemeLegacyRouteFacet, ThemeLegacyRouteId, ThemeLegacyRouteSelector, ThemeLegacyRouteValue,
};
pub(crate) use mechanisms::{
    collect_effect_graph_capabilities, paint_capabilities, paint_capability,
};
pub use merman_theme_contract::PresetExportV1;
pub use presets::{
    ThemePreset, ThemePresetDescriptor, ThemePresetParseError, ThemePresetQualifiedCell,
    theme_preset_descriptors,
};
pub(crate) use resolved::{
    ResolvedDiagramTheme, ResolvedProperty, ResolvedThemeEffect, ResolvedThemeStyle,
};
pub(crate) use resolved::{ResolvedStyleProperty, ThemeTypographyProperty};
pub use resources::{
    MAX_FONT_ALIASES_HARD_CAP, MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP,
    MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP, MAX_FONT_ASSETS_HARD_CAP,
    MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP, MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP,
    MAX_FONT_FACES_HARD_CAP, MAX_FONT_TABLES_HARD_CAP, MAX_THEME_BASE64_BYTES_HARD_CAP,
    MAX_THEME_ENCODED_BYTES_HARD_CAP, THEME_RESOURCE_LIMIT_COUNT, THEME_RESOURCE_LIMIT_DESCRIPTORS,
    ThemeResourceLimitDescriptor, ThemeResourceLimitExceeded, ThemeResourceLimitId,
    ThemeResourceLimitOverride, ThemeResourceLimitOverrideError, ThemeResourceLimitPhase,
    ThemeResourcePolicy, ThemeResourcePolicyRestrictionError, theme_resource_limit_descriptors,
};
pub use semantic::{
    OrdinalPalette, OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeEffectPatch,
    ThemeGeometryPatch, ThemePaintPatch, ThemeRule, ThemeRuleSet, ThemeSpacingPatch,
    ThemeStrokePatch, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
pub use spec::{DiagramThemeSpec, MermaidThemeCompatibility, MermaidThemeValue, ThemeAssets};
pub use support::{describe_theme_support, describe_theme_support_v2};
pub(crate) use typography::is_css_wide_keyword;
pub use typography::{
    FontStack, LineHeight, Specified, TextAlign, TextDecoration, TextStyle as ThemeTextStyle,
    TextStylePatch, TextTransform, TypographySpec, WhiteSpace, WrapMode as ThemeWrapMode,
};

pub(crate) use source_styles::{
    PreparedSourceStyleDeclaration, SourceStyleChannel, SourceStyleDeclaration, SourceStyleOrigin,
    SourceStyleProvenance, SourceStyleResidual, SourceStyleResidualReason,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeRecipeFingerprint([u8; 32]);

impl ThemeRecipeFingerprint {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl fmt::Debug for ThemeRecipeFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ThemeRecipeFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRecipeReport {
    theme_recipe_fingerprint: ThemeRecipeFingerprint,
    font_catalog_fingerprint: FontCatalogFingerprint,
    required_capabilities: BTreeSet<ThemeCapability>,
    required_text_capabilities: BTreeSet<TextLayoutCapability>,
}

impl ThemeRecipeReport {
    pub(crate) fn compiled(
        theme_recipe_fingerprint: ThemeRecipeFingerprint,
        font_catalog_fingerprint: FontCatalogFingerprint,
        required_capabilities: BTreeSet<ThemeCapability>,
        required_text_capabilities: BTreeSet<TextLayoutCapability>,
    ) -> Self {
        Self {
            theme_recipe_fingerprint,
            font_catalog_fingerprint,
            required_capabilities,
            required_text_capabilities,
        }
    }

    pub const fn theme_recipe_fingerprint(&self) -> ThemeRecipeFingerprint {
        self.theme_recipe_fingerprint
    }

    pub const fn font_catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub fn required_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required_capabilities.iter().copied()
    }

    pub fn requires_capability(&self, capability: ThemeCapability) -> bool {
        self.required_capabilities.contains(&capability)
    }

    pub fn required_text_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = TextLayoutCapability> + '_ {
        self.required_text_capabilities.iter().copied()
    }

    pub fn requires_text_capability(&self, capability: TextLayoutCapability) -> bool {
        self.required_text_capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone)]
pub struct DiagramTheme(pub(crate) Arc<CompiledDiagramTheme>);

impl DiagramTheme {
    /// Returns the content identity of the canonical recipe and its retained resources.
    ///
    /// This identity does not prove host admission, family evaluation, or output portability.
    pub fn recipe_fingerprint(&self) -> ThemeRecipeFingerprint {
        self.0.recipe_fingerprint
    }

    pub(crate) fn spec(&self) -> &DiagramThemeSpec {
        self.0.spec.as_ref()
    }

    pub(crate) fn resource_restriction(&self) -> &ThemeResourcePolicy {
        &self.0.resource_restriction
    }

    /// Revalidates compiled, retained theme resources under one session's effective policy.
    ///
    /// Input decoding limits have already served their purpose; output-dependent effect regions
    /// remain subject to family-local materialization checks.
    pub(crate) fn validate_retained_resources(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.0.catalog.validate_retained_resources(resources)?;
        self.0.spec.effects().check_resources(resources)
    }

    pub fn font_catalog(&self) -> &FontCatalog {
        &self.0.catalog
    }

    pub fn report(&self) -> &ThemeRecipeReport {
        &self.0.report
    }

    pub(crate) fn resolve(&self, family: crate::DiagramFamilyId) -> ResolvedDiagramTheme {
        ResolvedDiagramTheme::new(self.clone(), family)
    }

    fn family_program(
        &self,
        family: crate::DiagramFamilyId,
    ) -> Arc<family_program::FamilyThemeProgram> {
        self.0.family_programs.get_or_compile(family)
    }

    pub(crate) fn parse_compatibility(&self) -> &merman_core::__private::ThemeCompatibilityPlan {
        &self.0.parse_compatibility
    }

    #[cfg(test)]
    pub(crate) fn install_parse_compatibility(
        &self,
        engine: merman_core::Engine,
    ) -> merman_core::Engine {
        crate::__private::install_parse_compatibility(self, engine)
    }

    pub(crate) fn requirements(&self) -> &ThemeRequirements {
        &self.0.requirements
    }

    pub(crate) fn resolve_runtime_admission(
        &self,
        admission: &ThemeAdmissionPolicy,
        font_sources: &FontSourcePolicy,
        measurement_fallbacks: &HostMeasurementFallbackPolicy,
        portability: ThemePortabilityRequirement,
    ) -> Result<ResolvedThemeAdmission, ThemeAdmissionError> {
        admission::resolve_theme_admission(
            admission,
            font_sources,
            measurement_fallbacks,
            portability,
            self.requirements(),
            self.font_catalog(),
        )
    }
}

#[derive(Debug)]
pub(crate) struct CompiledDiagramTheme {
    spec: Arc<DiagramThemeSpec>,
    catalog: FontCatalog,
    resource_restriction: ThemeResourcePolicy,
    requirements: ThemeRequirements,
    parse_compatibility: merman_core::__private::ThemeCompatibilityPlan,
    family_programs: Arc<family_program::FamilyThemeProgramCache>,
    recipe_fingerprint: ThemeRecipeFingerprint,
    report: ThemeRecipeReport,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeCompileValidationError {
    #[error("theme field `{field}` must not be empty")]
    EmptyValue { field: &'static str },
    #[error("theme field `{field}` has an invalid value")]
    InvalidValue { field: &'static str },
    #[error("theme color `{value}` is invalid")]
    InvalidColor { value: String },
    #[error("theme field `{field}` must contain a finite value in its supported range")]
    InvalidNumber { field: &'static str },
    #[error("theme field `{field}` has an invalid collection shape")]
    InvalidCollection { field: &'static str },
    #[error("theme field `{field}` exceeds its implementation limit")]
    LimitExceeded { field: &'static str },
    #[error("theme field `{field}` contains a duplicate id")]
    DuplicateId { field: &'static str },
    #[error("theme field `{field}` references an unknown id")]
    UnknownId { field: &'static str },
    #[error("theme field `{field}` uses unsupported Mermaid theme `{value}`")]
    UnsupportedMermaidTheme { field: &'static str, value: String },
    #[error("theme target `{target}` is not valid for render family `{family}`")]
    InvalidTargetFamily {
        target: &'static str,
        family: &'static str,
    },
    #[error("canvas theme rules cannot be scoped to render family `{family}`")]
    InvalidCanvasFamilyScope { family: &'static str },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;

    #[test]
    fn inferred_capabilities_are_retained_as_theme_requirements() {
        let stops = [
            GradientStop::new(0.0, ThemeColorValue::parse("#000").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#fff").unwrap()).unwrap(),
        ];
        let spec = DiagramThemeSpec::new().with_canvas(CanvasSpec::default().with_base(
            CanvasPaint::LinearGradient(LinearGradient::new(45.0, stops).unwrap()),
        ));
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("host-independent recipe compilation");

        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::GradientPaint)
        );
    }

    #[test]
    fn resolved_style_honors_order_and_explicit_clear() {
        let first = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap());
        let mut second = ThemeStylePatch::default();
        second.paint.fill = Specified::Clear;
        second.typography.font_size_px = Specified::Value(22.0);
        let mut third = ThemeStylePatch::default();
        third.typography.font_size_px = Specified::Clear;
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, first))
            .with_rule(ThemeRule::new(ThemeTarget::Node, second))
            .with_rule(ThemeRule::new(ThemeTarget::Node, third));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();

        let style = theme.resolve(DiagramFamilyId::FLOWCHART).style(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
        );
        assert_eq!(style.fill(), None);
        assert_eq!(style.typography().font_size_px(), 16.0);
    }

    #[test]
    fn base_rules_inherit_into_named_variants_before_specific_overrides() {
        let mut base = ThemeStylePatch::default();
        base.paint.fill = Specified::Value(CanvasPaint::solid("#ef4444").expect("valid base fill"));
        base.typography.font_size_px = Specified::Value(22.0);
        let specific = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2563eb").unwrap());
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::SpecialState, base))
            .with_rule(
                ThemeRule::new(ThemeTarget::SpecialState, specific)
                    .with_variant(ThemeVariant::Start),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let resolved = theme.resolve(DiagramFamilyId::STATE);

        let default_style = resolved.style(ThemeTarget::SpecialState, ThemeVariant::Default, None);
        assert_eq!(
            default_style.fill().and_then(solid_color),
            Some("#ef4444".to_string())
        );
        assert_eq!(default_style.typography().font_size_px(), 22.0);

        let start_style = resolved.style(ThemeTarget::SpecialState, ThemeVariant::Start, None);
        assert_eq!(
            start_style.fill().and_then(solid_color),
            Some("#2563eb".to_string())
        );
        assert_eq!(start_style.typography().font_size_px(), 22.0);
    }

    #[test]
    fn invalid_family_target_and_unknown_effect_binding_fail_compilation() {
        let invalid_target = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Actor,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#fff").unwrap()),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        assert!(matches!(
            DiagramThemeCompiler::new().compile(invalid_target),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::InvalidTargetFamily { .. }
            ))
        ));

        let canvas_family_rule = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Canvas,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#fff").unwrap()),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        assert!(matches!(
            DiagramThemeCompiler::new().compile(canvas_family_rule),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::InvalidCanvasFamilyScope { .. }
            ))
        ));

        let effects = DiagramEffectSet::default()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "missing").unwrap())
            .unwrap();
        assert!(matches!(
            DiagramThemeCompiler::new().compile(DiagramThemeSpec::new().with_effects(effects)),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::UnknownId { .. }
            ))
        ));
    }

    fn solid_color(paint: &CanvasPaint) -> Option<String> {
        match paint {
            CanvasPaint::Solid(color) => Some(color.as_css()),
            _ => None,
        }
    }
}
