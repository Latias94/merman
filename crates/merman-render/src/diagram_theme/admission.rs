use std::collections::BTreeSet;

use super::assets::FontCatalog;

/// Typed visual capability that a compiled diagram theme may require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ThemeCapability {
    SemanticTokens,
    Typography,
    SemanticRules,
    OrdinalPalette,
    SolidPaint,
    TransparentPaint,
    GradientPaint,
    LayeredCanvas,
    PatternPaint,
    CanvasBleed,
    CanvasLayerPlacement,
    BlendMode,
    BorderStyling,
    DashStyling,
    RoundedGeometry,
    LetterSpacing,
    TextTransform,
    Shadow,
    SvgFilter,
    Noise,
    Displacement,
    ContentPadding,
    Opacity,
    TextDecoration,
    WhiteSpaceWrapping,
    WordSpacing,
}

impl ThemeCapability {
    pub const ALL: &'static [Self] = &[
        Self::SemanticTokens,
        Self::Typography,
        Self::SemanticRules,
        Self::OrdinalPalette,
        Self::SolidPaint,
        Self::TransparentPaint,
        Self::GradientPaint,
        Self::LayeredCanvas,
        Self::PatternPaint,
        Self::CanvasBleed,
        Self::CanvasLayerPlacement,
        Self::BlendMode,
        Self::BorderStyling,
        Self::DashStyling,
        Self::RoundedGeometry,
        Self::LetterSpacing,
        Self::TextTransform,
        Self::Shadow,
        Self::SvgFilter,
        Self::Noise,
        Self::Displacement,
        Self::ContentPadding,
        Self::Opacity,
        Self::TextDecoration,
        Self::WhiteSpaceWrapping,
        Self::WordSpacing,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::SemanticTokens => "semantic-tokens",
            Self::Typography => "typography",
            Self::SemanticRules => "semantic-rules",
            Self::OrdinalPalette => "ordinal-palette",
            Self::SolidPaint => "solid-paint",
            Self::TransparentPaint => "transparent-paint",
            Self::GradientPaint => "gradient-paint",
            Self::LayeredCanvas => "layered-canvas",
            Self::PatternPaint => "pattern-paint",
            Self::CanvasBleed => "canvas-bleed",
            Self::CanvasLayerPlacement => "canvas-layer-placement",
            Self::BlendMode => "blend-mode",
            Self::BorderStyling => "border-styling",
            Self::DashStyling => "dash-styling",
            Self::RoundedGeometry => "rounded-geometry",
            Self::LetterSpacing => "letter-spacing",
            Self::TextTransform => "text-transform",
            Self::Shadow => "shadow",
            Self::SvgFilter => "svg-filter",
            Self::Noise => "noise",
            Self::Displacement => "displacement",
            Self::ContentPadding => "content-padding",
            Self::Opacity => "opacity",
            Self::TextDecoration => "text-decoration",
            Self::WhiteSpaceWrapping => "white-space-wrapping",
            Self::WordSpacing => "word-spacing",
        }
    }
}

impl std::fmt::Display for ThemeCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Text-layout feature that a custom-font theme may require from its prepared backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TextLayoutCapability {
    CatalogBinding,
    UnicodeClusterFallback,
    OpenTypeShaping,
    Direction,
    Script,
    Language,
    FeatureSettings,
    VariationSettings,
}

impl TextLayoutCapability {
    pub const ALL: &'static [Self] = &[
        Self::CatalogBinding,
        Self::UnicodeClusterFallback,
        Self::OpenTypeShaping,
        Self::Direction,
        Self::Script,
        Self::Language,
        Self::FeatureSettings,
        Self::VariationSettings,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::CatalogBinding => "catalog-binding",
            Self::UnicodeClusterFallback => "unicode-cluster-fallback",
            Self::OpenTypeShaping => "opentype-shaping",
            Self::Direction => "direction",
            Self::Script => "script",
            Self::Language => "language",
            Self::FeatureSettings => "feature-settings",
            Self::VariationSettings => "variation-settings",
        }
    }
}

impl std::fmt::Display for TextLayoutCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Portability admission selected by the host, independently from font-source policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemePortabilityRequirement {
    #[default]
    BestEffort,
    RequirePortable,
}

impl ThemePortabilityRequirement {
    pub const fn stricter(self, other: Self) -> Self {
        match (self, other) {
            (Self::RequirePortable, _) | (_, Self::RequirePortable) => Self::RequirePortable,
            (Self::BestEffort, Self::BestEffort) => Self::BestEffort,
        }
    }
}

/// Source from which a text or export backend may resolve a font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FontSource {
    Embedded,
    System,
}

impl FontSource {
    pub const ALL: &'static [Self] = &[Self::Embedded, Self::System];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::System => "system",
        }
    }
}

impl std::fmt::Display for FontSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Ordered host-owned font-source policy.
///
/// The order is priority and the contained values are the allowed set. Restriction always keeps
/// the order of the receiver, so an operation cannot replace host priority with request priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSourcePolicy {
    priority: &'static [FontSource],
}

impl FontSourcePolicy {
    pub fn new(
        priority: impl IntoIterator<Item = FontSource>,
    ) -> Result<Self, ThemeAdmissionError> {
        let mut priority = priority.into_iter();
        let first = priority
            .next()
            .ok_or(ThemeAdmissionError::EmptyFontSourcePolicy)?;
        let Some(second) = priority.next() else {
            return Ok(match first {
                FontSource::Embedded => Self::embedded_only(),
                FontSource::System => Self::system_only(),
            });
        };
        if first == second {
            return Err(ThemeAdmissionError::DuplicateFontSource(second));
        }
        if let Some(duplicate) = priority.next() {
            return Err(ThemeAdmissionError::DuplicateFontSource(duplicate));
        }
        Ok(match first {
            FontSource::Embedded => Self::embedded_then_system(),
            FontSource::System => Self::system_then_embedded(),
        })
    }

    pub fn embedded_only() -> Self {
        Self {
            priority: &[FontSource::Embedded],
        }
    }

    pub fn system_only() -> Self {
        Self {
            priority: &[FontSource::System],
        }
    }

    pub fn embedded_then_system() -> Self {
        Self {
            priority: &[FontSource::Embedded, FontSource::System],
        }
    }

    pub fn system_then_embedded() -> Self {
        Self {
            priority: &[FontSource::System, FontSource::Embedded],
        }
    }

    pub fn priority(&self) -> impl ExactSizeIterator<Item = FontSource> + '_ {
        self.priority.iter().copied()
    }

    pub fn contains(&self, source: FontSource) -> bool {
        self.priority.contains(&source)
    }

    pub fn first(&self) -> FontSource {
        self.priority[0]
    }

    pub fn restrict_with(&self, restriction: &Self) -> Result<Self, ThemeAdmissionError> {
        self.restrict_to(restriction.priority.iter().copied())
    }

    pub fn restrict_to(
        &self,
        allowed: impl IntoIterator<Item = FontSource>,
    ) -> Result<Self, ThemeAdmissionError> {
        let mut embedded = false;
        let mut system = false;
        for source in allowed {
            match source {
                FontSource::Embedded => embedded = true,
                FontSource::System => system = true,
            }
        }
        match (
            embedded && self.contains(FontSource::Embedded),
            system && self.contains(FontSource::System),
        ) {
            (true, true) => Ok(self.clone()),
            (true, false) => Ok(Self::embedded_only()),
            (false, true) => Ok(Self::system_only()),
            (false, false) => Err(ThemeAdmissionError::EmptyFontSourceIntersection),
        }
    }
}

impl Default for FontSourcePolicy {
    fn default() -> Self {
        Self::embedded_then_system()
    }
}

/// Trusted compatibility lane that typed theme specs cannot enable by themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum TrustedThemeLane {
    RawThemeCss,
    ArbitrarySvg,
    SvgPostprocessor,
}

impl TrustedThemeLane {
    pub const ALL: &'static [Self] = &[
        Self::RawThemeCss,
        Self::ArbitrarySvg,
        Self::SvgPostprocessor,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::RawThemeCss => "raw-theme-css",
            Self::ArbitrarySvg => "arbitrary-svg",
            Self::SvgPostprocessor => "svg-postprocessor",
        }
    }

    pub(crate) const fn usage_mask(self) -> u64 {
        match self {
            Self::RawThemeCss => 1 << 0,
            Self::ArbitrarySvg => 1 << 1,
            Self::SvgPostprocessor => 1 << 2,
        }
    }
}

impl std::fmt::Display for TrustedThemeLane {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Set of trusted compatibility lanes explicitly enabled by a host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedThemeLanes {
    allowed: BTreeSet<TrustedThemeLane>,
}

impl TrustedThemeLanes {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            allowed: TrustedThemeLane::ALL.iter().copied().collect(),
        }
    }

    pub fn from_allowed(allowed: impl IntoIterator<Item = TrustedThemeLane>) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
        }
    }

    pub fn contains(&self, lane: TrustedThemeLane) -> bool {
        self.allowed.contains(&lane)
    }

    pub fn allowed(&self) -> impl ExactSizeIterator<Item = TrustedThemeLane> + '_ {
        self.allowed.iter().copied()
    }

    pub fn restrict_with(&self, restriction: &Self) -> Self {
        Self {
            allowed: self
                .allowed
                .intersection(&restriction.allowed)
                .copied()
                .collect(),
        }
    }
}

/// Minimum embedding behavior required by a theme from every retained face.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FontEmbeddingRequirement {
    #[default]
    NoEmbedding,
    FullFont,
    Subset,
}

/// Immutable theme-declared requirements. It contains no host priority or resource ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRequirements {
    required_capabilities: BTreeSet<ThemeCapability>,
    required_text_capabilities: BTreeSet<TextLayoutCapability>,
}

impl ThemeRequirements {
    pub fn new() -> Self {
        Self {
            required_capabilities: BTreeSet::new(),
            required_text_capabilities: BTreeSet::new(),
        }
    }

    pub fn with_required_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = ThemeCapability>,
    ) -> Self {
        self.required_capabilities.extend(capabilities);
        self
    }

    pub fn with_required_text_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = TextLayoutCapability>,
    ) -> Self {
        self.required_text_capabilities.extend(capabilities);
        self
    }

    pub fn required_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required_capabilities.iter().copied()
    }

    pub fn required_text_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = TextLayoutCapability> + '_ {
        self.required_text_capabilities.iter().copied()
    }
}

impl Default for ThemeRequirements {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ThemeCapabilities(u32);

impl Iterator for ThemeCapabilities {
    type Item = ThemeCapability;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let index = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(ThemeCapability::ALL[index])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.0.count_ones() as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ThemeCapabilities {}

impl std::fmt::Debug for ThemeCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(*self).finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TextLayoutCapabilities(u32);

impl Iterator for TextLayoutCapabilities {
    type Item = TextLayoutCapability;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let index = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(TextLayoutCapability::ALL[index])
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.0.count_ones() as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TextLayoutCapabilities {}

impl std::fmt::Debug for TextLayoutCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(*self).finish()
    }
}

/// Host-owned monotonic ceiling for typed theme and trusted-lane capabilities.
///
/// Font sources, host measurement fallbacks, and portability admission remain separate typed
/// policy axes. Keeping them out of this type prevents an acceptance decision from becoming a
/// disguised font-source or fallback policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeAdmissionPolicy {
    allowed_capabilities: ThemeCapabilities,
    allowed_text_capabilities: TextLayoutCapabilities,
    trusted_lanes: TrustedThemeLanes,
}

impl ThemeAdmissionPolicy {
    pub fn permissive() -> Self {
        Self {
            allowed_capabilities: ThemeCapabilities((1 << ThemeCapability::ALL.len()) - 1),
            allowed_text_capabilities: TextLayoutCapabilities(
                (1 << TextLayoutCapability::ALL.len()) - 1,
            ),
            trusted_lanes: TrustedThemeLanes::none(),
        }
    }

    pub fn with_allowed_capabilities(
        mut self,
        allowed: impl IntoIterator<Item = ThemeCapability>,
    ) -> Self {
        self.allowed_capabilities = ThemeCapabilities(
            allowed
                .into_iter()
                .fold(0, |mask, capability| mask | (1 << capability as u32)),
        );
        self
    }

    pub fn with_allowed_text_capabilities(
        mut self,
        allowed: impl IntoIterator<Item = TextLayoutCapability>,
    ) -> Self {
        self.allowed_text_capabilities = TextLayoutCapabilities(
            allowed
                .into_iter()
                .fold(0, |mask, capability| mask | (1 << capability as u32)),
        );
        self
    }

    pub fn with_trusted_lanes(mut self, lanes: TrustedThemeLanes) -> Self {
        self.trusted_lanes = lanes;
        self
    }

    pub fn allowed_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.allowed_capabilities
    }

    pub fn allowed_text_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = TextLayoutCapability> + '_ {
        self.allowed_text_capabilities
    }

    pub const fn trusted_lanes(&self) -> &TrustedThemeLanes {
        &self.trusted_lanes
    }

    /// Applies an operation-level restriction without allowing any host axis to be widened.
    pub fn restrict_with(&self, restriction: &Self) -> Self {
        Self {
            allowed_capabilities: ThemeCapabilities(
                self.allowed_capabilities.0 & restriction.allowed_capabilities.0,
            ),
            allowed_text_capabilities: TextLayoutCapabilities(
                self.allowed_text_capabilities.0 & restriction.allowed_text_capabilities.0,
            ),
            trusted_lanes: self.trusted_lanes.restrict_with(&restriction.trusted_lanes),
        }
    }
}

impl Default for ThemeAdmissionPolicy {
    fn default() -> Self {
        Self::permissive()
    }
}

/// Immutable result of resolving theme requirements against host policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedThemeAdmission {
    host_allowed_capabilities: BTreeSet<ThemeCapability>,
    host_allowed_text_capabilities: BTreeSet<TextLayoutCapability>,
    font_sources: FontSourcePolicy,
    portability: ThemePortabilityRequirement,
    trusted_lanes: TrustedThemeLanes,
}

pub(crate) fn resolve_theme_admission(
    admission: &ThemeAdmissionPolicy,
    font_sources: &FontSourcePolicy,
    portability: ThemePortabilityRequirement,
    requirements: &ThemeRequirements,
    catalog: &FontCatalog,
) -> Result<ResolvedThemeAdmission, ThemeAdmissionError> {
    if let Some(capability) = requirements
        .required_capabilities
        .iter()
        .find(|capability| admission.allowed_capabilities.0 & (1 << **capability as u32) == 0)
    {
        return Err(ThemeAdmissionError::ThemeCapabilityDenied(*capability));
    }
    if let Some(capability) = requirements
        .required_text_capabilities
        .iter()
        .find(|capability| admission.allowed_text_capabilities.0 & (1 << **capability as u32) == 0)
    {
        return Err(ThemeAdmissionError::TextLayoutCapabilityDenied(*capability));
    }

    Ok(ResolvedThemeAdmission {
        host_allowed_capabilities: requirements.required_capabilities.clone(),
        host_allowed_text_capabilities: requirements.required_text_capabilities.clone(),
        font_sources: font_sources.restrict_to(catalog.available_sources())?,
        portability,
        trusted_lanes: admission.trusted_lanes.clone(),
    })
}

impl ResolvedThemeAdmission {
    pub(crate) fn report(&self) -> ThemeHostAdmissionReport {
        ThemeHostAdmissionReport {
            host_allowed_capabilities: self.host_allowed_capabilities.clone(),
            host_allowed_text_capabilities: self.host_allowed_text_capabilities.clone(),
            font_sources: self.font_sources.clone(),
            portability: self.portability,
            trusted_lanes: self.trusted_lanes.clone(),
        }
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_sources
    }

    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability
    }

    pub(crate) const fn trusted_lanes(&self) -> &TrustedThemeLanes {
        &self.trusted_lanes
    }
}

/// Immutable evidence that one render environment allowed a recipe's requirements.
///
/// This report records host policy resolution only. Family application and target portability are
/// separate downstream evaluations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeHostAdmissionReport {
    host_allowed_capabilities: BTreeSet<ThemeCapability>,
    host_allowed_text_capabilities: BTreeSet<TextLayoutCapability>,
    font_sources: FontSourcePolicy,
    portability: ThemePortabilityRequirement,
    trusted_lanes: TrustedThemeLanes,
}

impl ThemeHostAdmissionReport {
    pub fn host_allowed_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.host_allowed_capabilities.iter().copied()
    }

    pub fn host_allowed_text_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = TextLayoutCapability> + '_ {
        self.host_allowed_text_capabilities.iter().copied()
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_sources
    }

    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability
    }

    pub const fn trusted_lanes(&self) -> &TrustedThemeLanes {
        &self.trusted_lanes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeAdmissionError {
    #[error("font-source policy must allow at least one source")]
    EmptyFontSourcePolicy,
    #[error("font-source policy contains duplicate source `{0}`")]
    DuplicateFontSource(FontSource),
    #[error("theme must declare at least one available font source")]
    EmptyThemeFontSources,
    #[error("theme and host font-source policies have no source in common")]
    EmptyFontSourceIntersection,
    #[error("theme capability `{0}` is disabled by the host")]
    ThemeCapabilityDenied(ThemeCapability),
    #[error("text-layout capability `{0}` is disabled by the host")]
    TextLayoutCapabilityDenied(TextLayoutCapability),
    #[error("trusted theme lane `{0}` is disabled by the host")]
    TrustedThemeLaneDenied(TrustedThemeLane),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_policy(values: &[FontSource]) -> FontSourcePolicy {
        FontSourcePolicy::new(values.iter().copied()).unwrap()
    }

    fn resolve(
        admission: &ThemeAdmissionPolicy,
        requirements: &ThemeRequirements,
    ) -> Result<ResolvedThemeAdmission, ThemeAdmissionError> {
        let catalog = FontCatalog::system_fonts();
        resolve_theme_admission(
            admission,
            &FontSourcePolicy::default(),
            ThemePortabilityRequirement::BestEffort,
            requirements,
            &catalog,
        )
    }

    #[test]
    fn font_source_intersection_is_complete_and_preserves_host_priority() {
        let non_empty_sets = [
            vec![FontSource::Embedded],
            vec![FontSource::System],
            vec![FontSource::Embedded, FontSource::System],
        ];

        for host_values in &non_empty_sets {
            for theme_values in &non_empty_sets {
                let host = source_policy(host_values);
                let actual = host.restrict_to(theme_values.iter().copied());
                let expected = host_values
                    .iter()
                    .copied()
                    .filter(|source| theme_values.contains(source))
                    .collect::<Vec<_>>();

                if expected.is_empty() {
                    assert_eq!(
                        actual,
                        Err(ThemeAdmissionError::EmptyFontSourceIntersection)
                    );
                } else {
                    assert_eq!(actual.unwrap().priority().collect::<Vec<_>>(), expected);
                }
            }
        }

        let host = FontSourcePolicy::system_then_embedded();
        let request = FontSourcePolicy::embedded_then_system();
        assert_eq!(
            host.restrict_with(&request)
                .unwrap()
                .priority()
                .collect::<Vec<_>>(),
            vec![FontSource::System, FontSource::Embedded]
        );
    }

    #[test]
    fn source_policies_reject_duplicates() {
        assert_eq!(
            FontSourcePolicy::new([]),
            Err(ThemeAdmissionError::EmptyFontSourcePolicy)
        );
        for source in FontSource::ALL.iter().copied() {
            assert_eq!(
                FontSourcePolicy::new([source, source]),
                Err(ThemeAdmissionError::DuplicateFontSource(source))
            );
            assert_eq!(
                FontSourcePolicy::new([FontSource::Embedded, FontSource::System, source]),
                Err(ThemeAdmissionError::DuplicateFontSource(source))
            );
        }
    }

    #[test]
    fn system_font_catalog_resolves_system_sources() {
        let resolved = resolve_theme_admission(
            &ThemeAdmissionPolicy::permissive(),
            &FontSourcePolicy::default(),
            ThemePortabilityRequirement::BestEffort,
            &ThemeRequirements::default(),
            &FontCatalog::system_fonts(),
        )
        .unwrap();

        assert_eq!(
            resolved.font_source_policy(),
            &FontSourcePolicy::system_only()
        );
    }

    #[test]
    fn policy_restriction_is_monotonic_on_every_axis() {
        let host = ThemeAdmissionPolicy::permissive().with_trusted_lanes(
            TrustedThemeLanes::from_allowed([
                TrustedThemeLane::RawThemeCss,
                TrustedThemeLane::SvgPostprocessor,
            ]),
        );
        let restriction = ThemeAdmissionPolicy::permissive()
            .with_allowed_capabilities([
                ThemeCapability::SemanticRules,
                ThemeCapability::Typography,
                ThemeCapability::SemanticRules,
            ])
            .with_allowed_text_capabilities([
                TextLayoutCapability::CatalogBinding,
                TextLayoutCapability::CatalogBinding,
            ])
            .with_trusted_lanes(TrustedThemeLanes::from_allowed([
                TrustedThemeLane::RawThemeCss,
                TrustedThemeLane::ArbitrarySvg,
            ]));

        let resolved = host.restrict_with(&restriction);

        let mut allowed = resolved.allowed_capabilities();
        assert_eq!(allowed.len(), 2);
        assert_eq!(allowed.next(), Some(ThemeCapability::Typography));
        assert_eq!(allowed.len(), 1);
        let mut allowed_text = resolved.allowed_text_capabilities();
        assert_eq!(allowed_text.len(), 1);
        assert_eq!(
            allowed_text.next(),
            Some(TextLayoutCapability::CatalogBinding)
        );
        assert_eq!(allowed_text.len(), 0);
        assert_eq!(
            resolved.allowed_capabilities().collect::<Vec<_>>(),
            vec![ThemeCapability::Typography, ThemeCapability::SemanticRules]
        );
        assert_eq!(
            resolved.allowed_text_capabilities().collect::<Vec<_>>(),
            vec![TextLayoutCapability::CatalogBinding]
        );
        assert_eq!(
            resolved.trusted_lanes().allowed().collect::<Vec<_>>(),
            vec![TrustedThemeLane::RawThemeCss]
        );

        assert_eq!(
            FontSourcePolicy::system_then_embedded()
                .restrict_with(&FontSourcePolicy::embedded_only())
                .unwrap()
                .priority()
                .collect::<Vec<_>>(),
            vec![FontSource::Embedded]
        );
        assert_eq!(
            ThemePortabilityRequirement::BestEffort
                .stricter(ThemePortabilityRequirement::RequirePortable),
            ThemePortabilityRequirement::RequirePortable
        );
    }

    #[test]
    fn theme_requirements_cannot_enable_denied_capabilities_or_sources() {
        let policy = ThemeAdmissionPolicy::permissive()
            .with_allowed_capabilities([ThemeCapability::SemanticTokens])
            .with_allowed_text_capabilities([]);

        let denied_theme =
            ThemeRequirements::new().with_required_capabilities([ThemeCapability::SvgFilter]);
        assert_eq!(
            resolve(&policy, &denied_theme),
            Err(ThemeAdmissionError::ThemeCapabilityDenied(
                ThemeCapability::SvgFilter
            ))
        );

        let denied_text = ThemeRequirements::new()
            .with_required_text_capabilities([TextLayoutCapability::OpenTypeShaping]);
        assert_eq!(
            resolve(&policy, &denied_text),
            Err(ThemeAdmissionError::TextLayoutCapabilityDenied(
                TextLayoutCapability::OpenTypeShaping
            ))
        );

        let denied_source = ThemeRequirements::new();
        let system_catalog = FontCatalog::system_fonts();
        assert_eq!(
            resolve_theme_admission(
                &policy,
                &FontSourcePolicy::embedded_only(),
                ThemePortabilityRequirement::RequirePortable,
                &denied_source,
                &system_catalog,
            ),
            Err(ThemeAdmissionError::EmptyFontSourceIntersection)
        );
    }

    #[test]
    fn every_theme_capability_is_admitted_only_by_set_intersection() {
        assert_eq!(
            ThemeAdmissionPolicy::permissive()
                .allowed_capabilities()
                .collect::<Vec<_>>(),
            ThemeCapability::ALL
        );
        for capability in ThemeCapability::ALL.iter().copied() {
            let requirements =
                ThemeRequirements::default().with_required_capabilities([capability]);
            let denied = ThemeAdmissionPolicy::permissive().with_allowed_capabilities([]);
            assert_eq!(
                resolve(&denied, &requirements),
                Err(ThemeAdmissionError::ThemeCapabilityDenied(capability))
            );

            let admitted =
                ThemeAdmissionPolicy::permissive().with_allowed_capabilities([capability]);
            assert!(resolve(&admitted, &requirements).is_ok());
        }
    }

    #[test]
    fn every_text_capability_is_admitted_only_by_set_intersection() {
        assert_eq!(
            ThemeAdmissionPolicy::permissive()
                .allowed_text_capabilities()
                .collect::<Vec<_>>(),
            TextLayoutCapability::ALL
        );
        for capability in TextLayoutCapability::ALL.iter().copied() {
            let requirements =
                ThemeRequirements::default().with_required_text_capabilities([capability]);
            let denied = ThemeAdmissionPolicy::permissive().with_allowed_text_capabilities([]);
            assert_eq!(
                resolve(&denied, &requirements),
                Err(ThemeAdmissionError::TextLayoutCapabilityDenied(capability))
            );

            let admitted =
                ThemeAdmissionPolicy::permissive().with_allowed_text_capabilities([capability]);
            assert!(resolve(&admitted, &requirements).is_ok());
        }
    }

    #[test]
    fn trusted_lane_intersection_is_complete() {
        for host_mask in 0_u8..8 {
            for restriction_mask in 0_u8..8 {
                let host_values = TrustedThemeLane::ALL
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(index, _)| host_mask & (1 << index) != 0)
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>();
                let restriction_values = TrustedThemeLane::ALL
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(index, _)| restriction_mask & (1 << index) != 0)
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>();
                let actual = TrustedThemeLanes::from_allowed(host_values.iter().copied())
                    .restrict_with(&TrustedThemeLanes::from_allowed(
                        restriction_values.iter().copied(),
                    ))
                    .allowed()
                    .collect::<Vec<_>>();
                let expected = host_values
                    .iter()
                    .copied()
                    .filter(|lane| restriction_values.contains(lane))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn stricter_portability_does_not_depend_on_enum_ordering() {
        assert_eq!(
            ThemePortabilityRequirement::BestEffort
                .stricter(ThemePortabilityRequirement::RequirePortable),
            ThemePortabilityRequirement::RequirePortable
        );
        assert_eq!(
            ThemePortabilityRequirement::RequirePortable
                .stricter(ThemePortabilityRequirement::BestEffort),
            ThemePortabilityRequirement::RequirePortable
        );
    }
}
