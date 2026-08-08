//! Portable, validated diagram-theme inputs and host admission policy.

mod admission;
mod assets;
mod canvas;
mod compiler;
mod effects;
mod mermaid_projection;
mod presets;
mod resolved;
mod resources;
mod semantic;
mod spec;
mod tokens;
mod typography;

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

pub use admission::{
    FontEmbeddingRequirement, FontSource, FontSourcePolicy, HostMeasurementFallback,
    HostMeasurementFallbackPolicy, TextLayoutCapability, ThemeAdmissionError, ThemeAdmissionPolicy,
    ThemeCapability, ThemePortabilityRequirement, ThemeRequirements, TrustedThemeLane,
    TrustedThemeLanes,
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
pub use compiler::{DiagramThemeCompiler, ThemeCapabilityReport, ThemeCompileError};
pub use effects::{
    DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FilterRegion,
};
pub use presets::{
    ThemePreset, ThemePresetDescriptor, ThemePresetParseError, theme_preset_descriptors,
};
pub use resolved::{ResolvedDiagramTheme, ResolvedThemeStyle};
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
pub use spec::{DiagramThemeSpec, MermaidThemeCompatibility, ThemeAssets};
pub use tokens::ThemeTokens;
pub use typography::{
    FontStack, LineHeight, Specified, TextAlign, TextDecoration, TextStyle as ThemeTextStyle,
    TextStylePatch, TextTransform, TypographySpec, WhiteSpace, WrapMode as ThemeWrapMode,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeFingerprint([u8; 32]);

impl ThemeFingerprint {
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

impl fmt::Debug for ThemeFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ThemeFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeResolutionReport {
    theme_fingerprint: ThemeFingerprint,
    font_catalog_fingerprint: FontCatalogFingerprint,
    required_capabilities: BTreeSet<ThemeCapability>,
}

impl ThemeResolutionReport {
    pub(crate) fn compiled(
        theme_fingerprint: ThemeFingerprint,
        font_catalog_fingerprint: FontCatalogFingerprint,
        required_capabilities: BTreeSet<ThemeCapability>,
    ) -> Self {
        Self {
            theme_fingerprint,
            font_catalog_fingerprint,
            required_capabilities,
        }
    }

    pub const fn theme_fingerprint(&self) -> ThemeFingerprint {
        self.theme_fingerprint
    }

    pub const fn font_catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub fn required_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required_capabilities.iter().copied()
    }
}

#[derive(Debug, Clone)]
pub struct DiagramTheme(pub(crate) Arc<CompiledDiagramTheme>);

impl PartialEq for DiagramTheme {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint() == other.fingerprint()
    }
}

impl Eq for DiagramTheme {}

impl DiagramTheme {
    pub fn fingerprint(&self) -> ThemeFingerprint {
        self.0.fingerprint
    }

    pub fn capabilities(&self) -> &ThemeCapabilityReport {
        &self.0.capabilities
    }

    pub fn spec(&self) -> &DiagramThemeSpec {
        &self.0.spec
    }

    pub fn font_catalog(&self) -> &FontCatalog {
        &self.0.catalog
    }

    pub fn report(&self) -> &ThemeResolutionReport {
        &self.0.report
    }

    pub fn resolve(&self, family: crate::family::RenderFamilyKind) -> ResolvedDiagramTheme {
        ResolvedDiagramTheme::new(self.clone(), family)
    }

    pub fn font_source_policy(&self) -> &FontSourcePolicy {
        self.0.admission.font_source_policy()
    }

    pub fn measurement_fallback_policy(&self) -> &HostMeasurementFallbackPolicy {
        self.0.admission.measurement_fallback_policy()
    }

    pub fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.0.admission.portability_requirement()
    }

    pub fn trusted_lanes(&self) -> &TrustedThemeLanes {
        self.0.admission.trusted_lanes()
    }

    pub fn mermaid_config(&self) -> &merman_core::MermaidConfig {
        &self.0.mermaid_config
    }
}

#[derive(Debug)]
pub(crate) struct CompiledDiagramTheme {
    spec: DiagramThemeSpec,
    catalog: FontCatalog,
    admission: admission::ResolvedThemeAdmission,
    mermaid_config: merman_core::MermaidConfig,
    capabilities: ThemeCapabilityReport,
    fingerprint: ThemeFingerprint,
    report: ThemeResolutionReport,
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
    #[error("theme target `{target}` is not valid for render family `{family}`")]
    InvalidTargetFamily {
        target: &'static str,
        family: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::RenderFamilyKind;

    #[test]
    fn tokens_compile_into_one_reusable_typed_theme() {
        let spec = ThemeTokens::default()
            .with_canvas("#0f172a")
            .unwrap()
            .with_surface("#1e293b")
            .unwrap()
            .with_text("#f8fafc")
            .unwrap()
            .into_theme_spec();

        let first = DiagramThemeCompiler::new().compile(spec.clone()).unwrap();
        let second = DiagramThemeCompiler::new().compile(spec).unwrap();

        assert_eq!(first, second);
        assert!(
            first
                .capabilities()
                .requires(ThemeCapability::SemanticRules)
        );
        assert!(
            first
                .capabilities()
                .requires(ThemeCapability::OrdinalPalette)
        );
        assert!(first.capabilities().requires(ThemeCapability::SolidCanvas));
        assert!(first.capabilities().requires(ThemeCapability::Typography));
        assert_eq!(
            first
                .mermaid_config()
                .get_str("themeVariables.primaryColor"),
            Some("#1e293b")
        );
        assert_eq!(
            first
                .mermaid_config()
                .get_str("themeVariables.stateLabelColor"),
            Some("#f8fafc")
        );

        let flowchart = first.resolve(RenderFamilyKind::Flowchart);
        assert_eq!(
            flowchart
                .style(ThemeTarget::Node, ThemeVariant::Default, None)
                .fill()
                .and_then(solid_color),
            Some("#1e293b".to_string())
        );
        assert_eq!(flowchart.series_color(ThemeTarget::ChartSeries, 5), None);
        let chart = first.resolve(RenderFamilyKind::XyChart);
        assert_eq!(
            chart
                .series_color(ThemeTarget::ChartSeries, 5)
                .map(ThemeColorValue::as_css),
            Some("#2563eb".to_string())
        );
    }

    #[test]
    fn inferred_capabilities_are_subject_to_host_admission() {
        let stops = [
            GradientStop::new(0.0, ThemeColorValue::parse("#000").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#fff").unwrap()).unwrap(),
        ];
        let spec = DiagramThemeSpec::new().with_canvas(CanvasSpec::default().with_base(
            CanvasPaint::LinearGradient(LinearGradient::new(45.0, stops).unwrap()),
        ));
        let compiler = DiagramThemeCompiler::new().with_admission(
            ThemeAdmissionPolicy::permissive()
                .with_allowed_capabilities([ThemeCapability::Typography]),
        );

        assert!(matches!(
            compiler.compile(spec),
            Err(ThemeCompileError::Admission(
                ThemeAdmissionError::ThemeCapabilityDenied(ThemeCapability::GradientCanvas)
            ))
        ));
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

        let style = theme.resolve(RenderFamilyKind::Flowchart).style(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
        );
        assert_eq!(style.fill(), None);
        assert_eq!(style.typography().font_size_px(), 16.0);
    }

    #[test]
    fn invalid_family_target_and_unknown_effect_binding_fail_compilation() {
        let invalid_target = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Actor,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#fff").unwrap()),
                )
                .for_family(RenderFamilyKind::Flowchart),
            ),
        );
        assert!(matches!(
            DiagramThemeCompiler::new().compile(invalid_target),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::InvalidTargetFamily { .. }
            ))
        ));

        let effects = DiagramEffectSet::default()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "missing"));
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
