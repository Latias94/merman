use merman_model::presentation::PresentationConfig;

use crate::config::config_theme_font_size_css_or_root_number_px_opt;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeOutcomes, InheritedFontStackPlan};

/// Resolves Architecture's inherited SVG typography without taking ownership of Cytoscape text.
///
/// Architecture has two intentionally separate font-size consumers. The base theme size applies
/// to SVG service/group/edge labels and CSS-inherited `iconText`; `architecture.fontSize` remains
/// the Cytoscape layout and group-icon-alignment owner. Keeping those values separate preserves
/// Mermaid's layout semantics.
#[derive(Debug)]
pub(crate) struct ArchitectureTypographyThemePlan {
    css_binding: super::super::ArchitectureCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeOutcomes,
}

/// Visibility observed by successful SVG text and icon-text writers.
#[derive(Debug, Default)]
pub(crate) struct ArchitectureTypographyEmission {
    visible: bool,
}

impl ArchitectureTypographyEmission {
    pub(crate) fn record_visible_text(&mut self) {
        self.visible = true;
    }
}

impl ArchitectureTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        // Only the explicit theme-variable path owns this property. A top-level `fontSize` is a
        // Mermaid-compatible fallback and must not shadow an active typed base size.
        let config_owns_font_size = typed_font_size_requested
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_px = match (theme, typed_font_size_active) {
            (Some(theme), true) => f64::from(theme.typography().font_size_px()).max(1.0),
            _ => config_theme_font_size_css_or_root_number_px_opt(effective_config.as_value())
                .unwrap_or(16.0)
                .max(1.0),
        };
        let font_size_px = crate::number_format::canonicalize_number(font_size_px).max(1.0);
        let font_size_css: Box<str> =
            format!("{}px", crate::number_format::canonical_number(font_size_px)).into_boxed_str();

        let css_binding = super::super::ArchitectureCssBinding::resolve(
            effective_config.as_value(),
            inherited_font_stack.font_family_css(),
            &font_size_css,
        );
        Self {
            css_binding,
            inherited_font_stack,
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            evidence: FamilyThemeOutcomes::default(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) const fn css_binding(&self) -> &super::super::ArchitectureCssBinding {
        &self.css_binding
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_emission(&self) -> Option<ArchitectureTypographyEmission> {
        self.typography_requested()
            .then(ArchitectureTypographyEmission::default)
    }

    pub(crate) fn finish_evidence(
        &self,
        emission: Option<ArchitectureTypographyEmission>,
    ) -> FamilyThemeOutcomes {
        let mut evidence = self.evidence.clone();
        let has_visible_terminals = emission.is_some_and(|emission| emission.visible);
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_terminals);

        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
            has_visible_terminals,
        );
        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
            has_visible_terminals,
        );
        evidence
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeOutcomes,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
        visible: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
            return;
        }

        if visible {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else {
            evidence.mark_not_applicable(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ArchitectureTypographyThemePlan;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use merman_model::presentation::PresentationConfig;

    #[test]
    fn typed_values_are_resolved_for_svg_labels_only() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("font stack"))
            .with_font_size_px(18.0)
            .expect("font size");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::ARCHITECTURE, typography),
                ),
            )
            .expect("compile Architecture theme")
            .resolve(DiagramFamilyId::ARCHITECTURE);
        let plan = ArchitectureTypographyThemePlan::resolve(
            Some(&theme),
            &PresentationConfig::empty_object(),
        );

        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_px(), 18.0);
        assert!(plan.begin_emission().is_some());
    }

    #[test]
    fn raw_config_without_ownership_keeps_the_typed_route() {
        let typography = ThemeTextStyle::default()
            .with_font_size_px(18.0)
            .expect("font size");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::ARCHITECTURE, typography),
                ),
            )
            .expect("compile Architecture theme")
            .resolve(DiagramFamilyId::ARCHITECTURE);
        let config = PresentationConfig::try_from_frozen_parts_controlled(
            merman_model::presentation::FrozenPresentationConfigParts::new(
                merman_model::MermaidConfig::from_value(serde_json::json!({
                    "themeVariables": { "fontSize": "22px" }
                })),
            ),
            &merman_model::resources::InputResourcePolicy::for_profile(
                merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
            &merman_model::OperationControl::new(),
        )
        .expect("frozen config construction should not cancel")
        .expect("unowned frozen values should be valid");
        let plan = ArchitectureTypographyThemePlan::resolve(Some(&theme), &config);

        // These frozen values deliberately declare no explicit configuration owners.
        assert_eq!(plan.font_size_px(), 18.0);
        assert!(plan.typed_font_size_active);
    }
}
