use std::collections::BTreeSet;
use std::sync::Arc;

use merman_core::MermaidConfig;
use sha2::{Digest, Sha256};

use super::admission::{
    FontSourcePolicy, HostMeasurementFallback, HostMeasurementFallbackPolicy, ThemeAdmissionError,
    ThemeAdmissionPolicy, ThemeCapability, ThemePortabilityRequirement, resolve_theme_admission,
};
use super::assets::{FontCatalog, FontCatalogError};
use super::canvas::CanvasPaint;
use super::effects::EffectPrimitive;
use super::resources::{ThemeResourceLimitExceeded, ThemeResourcePolicy};
use super::spec::DiagramThemeSpec;
use super::typography::Specified;
use super::{ThemeCompileValidationError, ThemeFingerprint, ThemeResolutionReport};

const THEME_FINGERPRINT_DOMAIN: &[u8] = b"merman-diagram-theme-v1";

/// Host-owned compiler for one complete typed theme value.
#[derive(Debug, Clone)]
pub struct DiagramThemeCompiler {
    admission: ThemeAdmissionPolicy,
    resources: ThemeResourcePolicy,
    font_sources: FontSourcePolicy,
    measurement_fallbacks: HostMeasurementFallbackPolicy,
    portability: ThemePortabilityRequirement,
}

impl Default for DiagramThemeCompiler {
    fn default() -> Self {
        Self {
            admission: ThemeAdmissionPolicy::default(),
            resources: ThemeResourcePolicy::default(),
            font_sources: FontSourcePolicy::default(),
            measurement_fallbacks: HostMeasurementFallbackPolicy::new([
                HostMeasurementFallback::NativeCatalog,
                HostMeasurementFallback::AcceptHostDependent,
                HostMeasurementFallback::VendoredDefault,
            ])
            .expect("static measurement fallback policy"),
            portability: ThemePortabilityRequirement::BestEffort,
        }
    }
}

impl DiagramThemeCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_admission(mut self, admission: ThemeAdmissionPolicy) -> Self {
        self.admission = admission;
        self
    }

    pub fn with_resource_policy(mut self, resources: ThemeResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub fn with_font_source_policy(mut self, font_sources: FontSourcePolicy) -> Self {
        self.font_sources = font_sources;
        self
    }

    pub fn with_measurement_fallbacks(
        mut self,
        measurement_fallbacks: HostMeasurementFallbackPolicy,
    ) -> Self {
        self.measurement_fallbacks = measurement_fallbacks;
        self
    }

    pub fn with_portability_requirement(
        mut self,
        portability: ThemePortabilityRequirement,
    ) -> Self {
        self.portability = portability;
        self
    }

    pub const fn resource_policy(&self) -> &ThemeResourcePolicy {
        &self.resources
    }

    /// Rejects an encoded theme input before its typed object graph is allocated.
    pub fn check_encoded_input_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.resources.check_theme_encoded_bytes(actual)
    }

    pub fn compile(
        &self,
        spec: DiagramThemeSpec,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        spec.validate()?;
        let catalog = match spec.assets().font_catalog() {
            Some(catalog) => catalog.clone().compile(&self.resources)?,
            None => FontCatalog::default_parity(),
        };
        let inferred_capabilities = infer_required_capabilities(&spec);
        let effective_requirements = spec
            .requirements()
            .clone()
            .with_required_capabilities(inferred_capabilities);
        let admission = resolve_theme_admission(
            &self.admission,
            &self.font_sources,
            &self.measurement_fallbacks,
            self.portability,
            &effective_requirements,
            &catalog,
        )?;
        let mermaid_config = compile_mermaid_config(&spec);
        let capabilities = ThemeCapabilityReport::from_requirements(&effective_requirements);
        let fingerprint = fingerprint(&spec, &catalog, &mermaid_config);
        let report = ThemeResolutionReport::compiled(
            ThemeFingerprint::from_bytes(fingerprint),
            catalog.fingerprint(),
            capabilities.required.clone(),
        );
        Ok(super::DiagramTheme(Arc::new(super::CompiledDiagramTheme {
            spec,
            catalog,
            admission,
            mermaid_config,
            capabilities,
            fingerprint: ThemeFingerprint::from_bytes(fingerprint),
            report,
        })))
    }

    pub fn compile_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        self.compile(preset.spec())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCapabilityReport {
    required: BTreeSet<ThemeCapability>,
    admitted: BTreeSet<ThemeCapability>,
}

impl ThemeCapabilityReport {
    fn from_requirements(requirements: &super::ThemeRequirements) -> Self {
        let required = requirements
            .required_capabilities()
            .collect::<BTreeSet<_>>();
        let admitted = required.clone();
        Self { required, admitted }
    }

    pub fn required(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required.iter().copied()
    }

    pub fn admitted(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.admitted.iter().copied()
    }

    pub fn requires(&self, capability: ThemeCapability) -> bool {
        self.required.contains(&capability)
    }
}

fn infer_required_capabilities(spec: &DiagramThemeSpec) -> BTreeSet<ThemeCapability> {
    let mut required = BTreeSet::new();
    if !spec.styles().rules().is_empty() {
        required.insert(ThemeCapability::SemanticTokens);
        required.insert(ThemeCapability::SemanticRules);
    }
    if !spec.styles().ordinal_palettes().is_empty() {
        required.insert(ThemeCapability::OrdinalPalette);
    }
    if spec.typography() != &super::typography::TypographySpec::default() {
        required.insert(ThemeCapability::Typography);
    }
    collect_paint_capabilities(spec.canvas().base(), &mut required);
    for layer in spec.canvas().layers() {
        collect_paint_capabilities(layer.paint(), &mut required);
    }
    if !spec.canvas().layers().is_empty() {
        required.insert(ThemeCapability::LayeredCanvas);
    }
    if spec
        .canvas()
        .layers()
        .iter()
        .any(|layer| !matches!(layer.blend_mode(), super::BlendMode::Normal))
    {
        required.insert(ThemeCapability::BlendMode);
    }
    if !spec.effects().graphs().is_empty() {
        required.insert(ThemeCapability::SvgFilter);
        for graph in spec.effects().graphs() {
            for primitive in graph.primitives() {
                match primitive {
                    EffectPrimitive::DropShadow { .. } => {
                        required.insert(ThemeCapability::Shadow);
                    }
                    EffectPrimitive::Turbulence { .. } => {
                        required.insert(ThemeCapability::Noise);
                    }
                    EffectPrimitive::Displacement { .. } => {
                        required.insert(ThemeCapability::Displacement);
                    }
                    _ => {}
                }
            }
        }
    }
    for rule in spec.styles().rules() {
        let style = rule.style();
        if !matches!(style.stroke.width, Specified::Unspecified)
            || !matches!(style.stroke.linecap, Specified::Unspecified)
            || !matches!(style.stroke.linejoin, Specified::Unspecified)
        {
            required.insert(ThemeCapability::BorderStyling);
        }
        if !matches!(style.stroke.dasharray, Specified::Unspecified) {
            required.insert(ThemeCapability::DashStyling);
        }
        if !matches!(style.geometry.radius, Specified::Unspecified) {
            required.insert(ThemeCapability::RoundedGeometry);
        }
        if !matches!(style.typography.letter_spacing_px, Specified::Unspecified) {
            required.insert(ThemeCapability::Typography);
            required.insert(ThemeCapability::LetterSpacing);
        }
        if !matches!(style.typography.transform, Specified::Unspecified) {
            required.insert(ThemeCapability::Typography);
            required.insert(ThemeCapability::TextTransform);
        }
        if !matches!(style.typography.word_spacing_px, Specified::Unspecified) {
            required.insert(ThemeCapability::Typography);
            required.insert(ThemeCapability::WordSpacing);
        }
        if !matches!(style.typography.decoration, Specified::Unspecified) {
            required.insert(ThemeCapability::Typography);
            required.insert(ThemeCapability::TextDecoration);
        }
        if !matches!(style.typography.white_space, Specified::Unspecified)
            || !matches!(style.typography.wrap, Specified::Unspecified)
        {
            required.insert(ThemeCapability::Typography);
            required.insert(ThemeCapability::WhiteSpaceWrapping);
        }
        if style.typography != super::TextStylePatch::default() {
            required.insert(ThemeCapability::Typography);
        }
        if !matches!(style.spacing.padding, Specified::Unspecified) {
            required.insert(ThemeCapability::ContentPadding);
        }
        if !matches!(style.paint.opacity, Specified::Unspecified)
            || !matches!(style.paint.fill_opacity, Specified::Unspecified)
            || !matches!(style.stroke.stroke_opacity, Specified::Unspecified)
        {
            required.insert(ThemeCapability::Opacity);
        }
    }
    required
}

fn collect_paint_capabilities(paint: &CanvasPaint, required: &mut BTreeSet<ThemeCapability>) {
    match paint {
        CanvasPaint::Transparent => {}
        CanvasPaint::Solid(_) => {
            required.insert(ThemeCapability::SolidCanvas);
        }
        CanvasPaint::LinearGradient(_) | CanvasPaint::RadialGradient(_) => {
            required.insert(ThemeCapability::GradientCanvas);
        }
        CanvasPaint::Pattern(_) => {
            required.insert(ThemeCapability::PatternCanvas);
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeCompileError {
    #[error(transparent)]
    Validation(#[from] ThemeCompileValidationError),
    #[error(transparent)]
    Admission(#[from] ThemeAdmissionError),
    #[error(transparent)]
    FontCatalog(#[from] FontCatalogError),
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
}

fn compile_mermaid_config(spec: &DiagramThemeSpec) -> MermaidConfig {
    super::mermaid_projection::compile(spec)
}

fn fingerprint(
    spec: &DiagramThemeSpec,
    catalog: &FontCatalog,
    mermaid_config: &MermaidConfig,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, THEME_FINGERPRINT_DOMAIN);
    update_len_prefixed(&mut hasher, format!("{spec:?}").as_bytes());
    update_len_prefixed(&mut hasher, catalog.fingerprint().as_bytes());
    if let Ok(value) = serde_json::to_vec(mermaid_config.as_value()) {
        update_len_prefixed(&mut hasher, &value);
    }
    hasher.finalize().into()
}

fn update_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}
