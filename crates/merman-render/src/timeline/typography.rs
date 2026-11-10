use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};

/// Resolves Timeline's base typography once for layout, CSS emission, and evidence.
#[derive(Debug)]
pub(crate) struct TimelineTypographyThemePlan {
    css_binding: super::TimelineCssBinding,
    common_css: crate::svg::PreparedCommonCss,
    layout_settings: super::config::TimelineLayoutSettings,
    inherited_font_stack: InheritedFontStackPlan,
    used_font_size_px: Option<f64>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    terminal: OnceLock<TimelineTypographyOutcome>,
}

#[derive(Debug, Default)]
pub(crate) struct TimelineTypographyOutcome {
    has_visible_node_text: bool,
    has_visible_title: bool,
    has_unverified_node_text: bool,
    has_unverified_title: bool,
}

impl TimelineTypographyThemePlan {
    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
    ) -> Self {
        Self::resolve_with_text(theme, effective_config, None)
    }

    pub(crate) fn resolve_with_text(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        text_fill: Option<&str>,
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
        // Root `fontSize` remains Timeline's independent layout-offset owner. Only an explicit
        // theme-variable value shadows the typed base-size route itself.
        let configured_font_size =
            super::TimelineConfigView::new(effective_config.as_value()).configured_font_size();
        let config_owns_font_size = typed_font_size_requested
            && configured_font_size.is_some()
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size = match theme {
            Some(theme) if typed_font_size_active => {
                let font_size_px = theme.typography().font_size_px();
                super::config::TimelineFontSize {
                    css: format!("{font_size_px}px"),
                    used_px: Some(f64::from(font_size_px)),
                }
            }
            _ => configured_font_size.unwrap_or_default(),
        };

        let mut common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            effective_config.as_value(),
            inherited_font_stack.font_family_css().to_owned(),
            font_size.css,
        );
        if let Some(fill) = text_fill {
            common_css = common_css.with_text_color(fill);
        }
        let layout_settings = super::TimelineConfigView::new(effective_config.as_value())
            .layout_settings_with_resolved_typography(
                Some(inherited_font_stack.font_family_css()),
                Some(font_size.used_px.unwrap_or(16.0)),
            );
        Self {
            css_binding: super::TimelineCssBinding::resolve(effective_config.as_value()),
            common_css,
            layout_settings,
            inherited_font_stack,
            used_font_size_px: font_size.used_px,
            typed_font_size_requested,
            typed_font_size_active,
            terminal: OnceLock::new(),
        }
    }

    pub(crate) fn css_binding(&self) -> &super::TimelineCssBinding {
        &self.css_binding
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn layout_settings(&self) -> &super::config::TimelineLayoutSettings {
        &self.layout_settings
    }

    pub(crate) const fn used_font_size_px(&self) -> Option<f64> {
        self.used_font_size_px
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_emission(&self) -> Option<TimelineTypographyOutcome> {
        self.typography_requested()
            .then(TimelineTypographyOutcome::default)
    }

    pub(crate) fn record_emission(&self, outcome: TimelineTypographyOutcome) {
        let _ = self.terminal.set(outcome);
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.typography_requested() {
            return;
        }

        let outcome = self.terminal.get();
        for property in self
            .inherited_font_stack
            .unsupported_typography_properties()
        {
            let has_visible_consumer = outcome.is_none_or(|outcome| match property {
                // The title owns its bold weight independently of inherited node typography.
                ThemeTypographyProperty::FontWeight => {
                    outcome.has_visible_node_text || outcome.has_unverified_node_text
                }
                _ => {
                    outcome.has_visible_node_text
                        || outcome.has_visible_title
                        || outcome.has_unverified_node_text
                        || outcome.has_unverified_title
                }
            });
            let key = FamilyThemeMechanismKey::Typography(property);
            if has_visible_consumer {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        let Some(outcome) = outcome else {
            for (property, requested, active) in [
                (
                    ThemeTypographyProperty::FontStack,
                    self.inherited_font_stack.typed_font_stack_requested(),
                    self.inherited_font_stack.typed_font_stack_active(),
                ),
                (
                    ThemeTypographyProperty::FontSize,
                    self.typed_font_size_requested,
                    self.typed_font_size_active,
                ),
            ] {
                if !requested {
                    continue;
                }
                let key = FamilyThemeMechanismKey::Typography(property);
                if active {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                } else {
                    evidence.mark_not_applicable(key);
                }
            }
            return;
        };

        for (property, requested, active, has_visible_consumer, has_unverified_consumer) in [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
                outcome.has_visible_node_text || outcome.has_visible_title,
                outcome.has_unverified_node_text || outcome.has_unverified_title,
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
                // The title's explicit 4ex size is not an inherited font-size declaration.
                outcome.has_visible_node_text,
                outcome.has_unverified_node_text,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if !active {
                evidence.mark_not_applicable(key);
            } else if has_unverified_consumer
                || (outcome.has_visible_node_text
                    && !self.css_binding.section_font_weight_is_measurable())
            {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else if has_visible_consumer {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

impl TimelineTypographyOutcome {
    pub(crate) fn record_base_text_run(&mut self, text: &str, font_size_px: Option<f64>) {
        if !text.trim().is_empty() {
            self.has_visible_node_text |= font_size_px.is_some_and(|size| size > 0.0);
            self.has_unverified_node_text |= font_size_px.is_none();
        }
    }

    pub(crate) fn record_title(&mut self, text: &str, parent_font_size_px: Option<f64>) {
        if !text.trim().is_empty() {
            self.has_visible_title |= parent_font_size_px.is_some_and(|size| size > 0.0);
            self.has_unverified_title |= parent_font_size_px.is_none();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_layout_preserves_independent_root_font_offset() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "redux", "fontSize": "18px",
            "themeVariables": {"fontSize": "24px", "fontWeight": 900}
        }));
        let plan =
            TimelineTypographyThemePlan::resolve_with_text(None, &config, Some("var(--text)"));
        assert_eq!(plan.layout_settings().text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().node_text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().text_style.font_weight, None);
        assert_eq!(
            plan.layout_settings()
                .node_text_style
                .font_weight
                .as_deref(),
            plan.css_binding().section_font_weight()
        );
        assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        assert_eq!(plan.common_css().text_color(), "var(--text)");
        assert_eq!(
            plan.layout_settings().text_style.font_family.as_deref(),
            Some(plan.inherited_font_stack.font_family_css())
        );
        assert_eq!(
            plan.layout_settings().node_text_style.font_family,
            plan.layout_settings().text_style.font_family
        );
        for size in [0.0, 0.5] {
            let config = crate::test_support::presentation_config(serde_json::json!({
                "fontSize": "18px", "themeVariables": {"fontSize": format!("{size}px")}
            }));
            let plan = TimelineTypographyThemePlan::resolve(None, &config);
            assert_eq!(plan.layout_settings().text_style.font_size, size);
            assert_eq!(plan.layout_settings().node_text_style.font_size, size);
            assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        }
    }
}
