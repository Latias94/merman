use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_theme_font_size_css_or_root_number_px_opt;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};
use crate::model::JourneyDiagramLayout;

/// Resolves Journey's inherited base typography once for layout, CSS, and evidence.
///
/// Journey task and title typography are intentionally not part of this plan. Mermaid gives
/// those roles their own configuration paths and the native fallback text writer emits them
/// explicitly.
#[derive(Debug)]
pub(crate) struct JourneyTypographyThemePlan {
    css: super::JourneyCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeEvidence,
    terminal_seal: OnceLock<JourneyTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct JourneyTypographyTerminalSeal {
    has_visible_base_text: bool,
}

/// Writer-owned proof for Journey's inherited stylesheet and visible text candidates.
#[derive(Debug)]
pub(crate) struct JourneyTypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    expected_text_runs: Box<[&'a str]>,
    next_text_run: usize,
    css_emitted: bool,
    css_matches: bool,
    has_visible_base_text: bool,
    valid: bool,
}

impl JourneyTypographyThemePlan {
    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self::resolve_with_text_paint(theme, effective_config, None)
    }

    pub(crate) fn resolve_with_text_paint(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
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
        // Mermaid's inherited base-size owner is the theme-variable path. A top-level `fontSize`
        // remains a compatibility fallback for the unthemed path and must not shadow a typed
        // family theme value.
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let configured_font_size =
            config_theme_font_size_css_or_root_number_px_opt(effective_config.as_value());
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let (font_size_px, font_size_css) = match (theme, typed_font_size_active) {
            (Some(theme), true) => {
                let font_size_px = f64::from(theme.typography().font_size_px()).max(1.0);
                (font_size_px, format!("{font_size_px}px").into_boxed_str())
            }
            (_, false) | (None, true) => {
                let font_size_px = configured_font_size.unwrap_or(16.0).max(1.0);
                (font_size_px, format!("{font_size_px}px").into_boxed_str())
            }
        };

        let css = super::JourneyCssBinding::new(
            effective_config.as_value(),
            inherited_font_stack.font_family_css(),
            &font_size_css,
            text_fill,
        );
        Self {
            css,
            inherited_font_stack,
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.css.common.font_family()
    }

    pub(crate) fn css_binding(&self) -> &super::JourneyCssBinding {
        &self.css
    }

    pub(crate) fn font_size_css(&self) -> &str {
        self.css.common.font_size_css()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a JourneyDiagramLayout,
    ) -> Option<JourneyTypographyThemeReceipt<'a>> {
        self.typography_requested().then(|| {
            JourneyTypographyThemeReceipt::new(
                self.font_family_css(),
                self.font_size_css(),
                journey_inherited_text_runs(layout),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_seal.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.typography_requested() {
            return evidence;
        }

        let Some(seal) = self.terminal_seal.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, true);
            self.mark_typed_property_evidence(
                &mut evidence,
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
                false,
                false,
            );
            self.mark_typed_property_evidence(
                &mut evidence,
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
                false,
                false,
            );
            return evidence;
        };

        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, seal.has_visible_base_text);
        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
            seal.has_visible_base_text,
            true,
        );
        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
            seal.has_visible_base_text,
            true,
        );
        evidence
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
        has_visible_base_text: bool,
        terminal_proved: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
        } else if !terminal_proved {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
        } else if !has_visible_base_text {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        }
    }
}

impl<'a> JourneyTypographyThemeReceipt<'a> {
    fn new(
        expected_font_family_css: &'a str,
        expected_font_size_css: &'a str,
        expected_text_runs: Vec<&'a str>,
    ) -> Self {
        Self {
            expected_font_family_css,
            expected_font_size_css,
            expected_text_runs: expected_text_runs.into_boxed_slice(),
            next_text_run: 0,
            css_emitted: false,
            css_matches: false,
            has_visible_base_text: false,
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(
        &mut self,
        emitted_font_family_css: &str,
        emitted_font_size_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
    ) {
        self.valid &= !self.css_emitted
            && base_typography_emitted
            && root_typography_emitted
            && emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
        self.css_emitted = true;
        self.css_matches = emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
    }

    pub(crate) fn record_text_run(&mut self, text: &str) {
        let expected = self.expected_text_runs.get(self.next_text_run).copied();
        self.valid &= expected == Some(text);
        if expected.is_some() {
            self.next_text_run = self.next_text_run.saturating_add(1);
        }
        self.has_visible_base_text |= !text.trim().is_empty();
    }

    fn seal(self) -> Option<JourneyTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.next_text_run == self.expected_text_runs.len())
        .then_some(JourneyTypographyTerminalSeal {
            has_visible_base_text: self.has_visible_base_text,
        })
    }
}

fn journey_inherited_text_runs(layout: &JourneyDiagramLayout) -> Vec<&str> {
    layout
        .actor_legend
        .iter()
        .flat_map(|item| item.label_lines.iter().map(|line| line.text.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{DiagramThemeCompiler, DiagramThemeSpec, FontStack, TypographySpec};
    use crate::model::{Bounds, JourneyDiagramLayout};

    fn empty_layout() -> JourneyDiagramLayout {
        JourneyDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 1.0,
                max_y: 1.0,
            }),
            left_margin: 0.0,
            max_actor_label_width: 0.0,
            width: 1.0,
            height: 1.0,
            svg_height: 1.0,
            use_max_width: false,
            title: None,
            title_x: 0.0,
            title_y: 0.0,
            actor_legend: Vec::new(),
            sections: Vec::new(),
            tasks: Vec::new(),
            activity_line: crate::model::JourneyLineLayout {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
        }
    }

    #[test]
    fn empty_receipt_seals_without_visible_base_text() {
        let layout = empty_layout();
        let typography = crate::diagram_theme::ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Open Sans").expect("font stack"))
            .with_font_size_px(16.0)
            .expect("font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(crate::DiagramFamilyId::JOURNEY, typography),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile empty theme")
            .resolve(crate::DiagramFamilyId::JOURNEY);
        let plan =
            JourneyTypographyThemePlan::resolve(Some(&theme), &MermaidConfig::empty_object());
        let mut receipt = plan.begin_terminal_receipt(&layout).expect("theme receipt");
        receipt.record_css_emission("\"Open Sans\"", "16px", true, true);
        assert!(plan.record_terminal(receipt));
    }

    #[test]
    fn typed_font_size_uses_theme_value_when_config_does_not_own_it() {
        let typography = crate::diagram_theme::ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("font stack"))
            .with_font_size_px(18.0)
            .expect("font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(crate::DiagramFamilyId::JOURNEY, typography),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile Journey theme")
            .resolve(crate::DiagramFamilyId::JOURNEY);
        let plan =
            JourneyTypographyThemePlan::resolve(Some(&theme), &MermaidConfig::empty_object());
        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_css(), "18px");
        assert_eq!(plan.font_size_px(), 18.0);
    }
}
