use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::config::{config_f64_css_absolute_px, config_theme_font_size_css_or_root_number_px_opt};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};
use crate::mermaid_style::CssFontFamilyOwnership;
use crate::model::JourneyDiagramLayout;

use super::config::JourneyRenderSettings;

/// Resolves Journey's inherited base typography once for layout, CSS, and evidence.
///
/// The legend and the `fo` / `old` label candidates inherit base typography. The `tspan`
/// candidate can inherit the font stack while retaining its separate task-size owner.
/// The title has separate source configuration owners.
#[derive(Debug)]
pub(crate) struct JourneyTypographyThemePlan {
    css: super::JourneyCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    terminal_seal: OnceLock<JourneyTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct JourneyTypographyTerminalSeal {
    has_visible_text: bool,
    has_visible_inherited_font_weight: bool,
    has_visible_font_stack: bool,
    has_unverified_font_stack: bool,
    has_visible_font_size: bool,
}

#[derive(Debug, Clone, Copy)]
struct JourneyTypographyTextRun<'a> {
    text: &'a str,
    visibility: Option<bool>,
    font_family_ownership: CssFontFamilyOwnership,
    inherits_font_size: bool,
    inherits_font_weight: bool,
}

/// Writer-owned proof for Journey's inherited stylesheet and visible text candidates.
#[derive(Debug)]
pub(crate) struct JourneyTypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    expected_text_runs: Box<[JourneyTypographyTextRun<'a>]>,
    next_text_run: usize,
    css_emitted: bool,
    css_matches: bool,
    outcome: JourneyTypographyTerminalSeal,
    valid: bool,
}

impl JourneyTypographyThemePlan {
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
        // Mermaid's inherited base-size owner is the theme-variable path. A top-level `fontSize`
        // remains a compatibility fallback for the unthemed path and must not shadow a typed
        // family theme value.
        let config_owns_font_size = typed_font_size_requested
            && config_f64_css_absolute_px(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .filter(|size| *size >= 0.0)
            .is_some()
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let configured_font_size =
            config_theme_font_size_css_or_root_number_px_opt(effective_config.as_value())
                .filter(|size| *size >= 0.0);
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let (font_size_px, font_size_css) = match (theme, typed_font_size_active) {
            (Some(theme), true) => {
                let font_size_px = f64::from(theme.typography().font_size_px());
                (font_size_px, format!("{font_size_px}px").into_boxed_str())
            }
            (_, false) | (None, true) => {
                let font_size_px = configured_font_size.unwrap_or(16.0).max(0.0);
                (font_size_px, format!("{font_size_px}px").into_boxed_str())
            }
        };

        let css = super::JourneyCssBinding::new(
            effective_config.as_value(),
            inherited_font_stack.font_family_css(),
            &font_size_css,
        );
        Self {
            css,
            inherited_font_stack,
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn with_text_paint(mut self, fill: Option<&str>) -> Self {
        self.css = self.css.with_text_paint(fill);
        self
    }

    pub(crate) fn title_can_be_visible(&self) -> bool {
        self.title_visibility() != Some(false)
    }

    fn title_visibility(&self) -> Option<bool> {
        if self.css.render.title_inherits_font_size() {
            return Some(self.font_size_px > 0.0);
        }
        if crate::mermaid_style::svg_font_size_is_zero_at_parent(
            &self.css.render.title_font_size,
            self.font_size_px,
        ) {
            return Some(false);
        }
        let raw = self.css.render.title_font_size.trim();
        let declaration = format!("font-size:{raw}");
        let parsed = crate::mermaid_style::parse_style_declaration(&declaration)?;
        if parsed.analysis().depends_on_root_font_size()
            || parsed.analysis().has_dynamic_reference_function()
        {
            return None;
        }
        if let Some(size) = parsed.resolve_font_size_px(
            crate::mermaid_style::CssFontSizeContext::uniform(self.font_size_px),
        ) {
            return Some(size > 0.0);
        }
        // Font-relative metrics are positive with a positive parent, even though their exact
        // lengths need the selected face. Expressions and document-root units remain unknown.
        let mut parser = cssparser::Parser::new(raw);
        match parser.next() {
            Ok(cssparser::Token::Dimension { value, unit, .. })
                if value.is_finite()
                    && *value > 0.0
                    && ["ex", "ch", "cap", "ic"]
                        .iter()
                        .any(|candidate| unit.eq_ignore_ascii_case(candidate)) =>
            {
                Some(self.font_size_px > 0.0)
            }
            _ => None,
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
        title: Option<&'a str>,
    ) -> Option<JourneyTypographyThemeReceipt<'a>> {
        self.typography_requested().then(|| {
            JourneyTypographyThemeReceipt::new(
                self.font_family_css(),
                self.font_size_css(),
                journey_text_runs(
                    layout,
                    &self.css.render,
                    self.font_size_px,
                    title,
                    self.title_visibility(),
                ),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_seal.set(seal).is_ok())
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.typography_requested() {
            return;
        }

        let seal = self.terminal_seal.get();
        for property in self
            .inherited_font_stack
            .unsupported_typography_properties()
        {
            let has_visible_consumer = seal.is_none_or(|seal| match property {
                // The title's explicit bold weight overrides the inherited base weight.
                ThemeTypographyProperty::FontWeight => seal.has_visible_inherited_font_weight,
                _ => seal.has_visible_text,
            });
            let key = FamilyThemeMechanismKey::Typography(property);
            if has_visible_consumer {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        let Some(seal) = seal else {
            self.mark_typed_property_evidence(
                evidence,
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
                false,
                false,
            );
            self.mark_typed_property_evidence(
                evidence,
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
                false,
                false,
            );
            return;
        };

        self.mark_typed_property_evidence(
            evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
            seal.has_visible_font_stack,
            seal.has_unverified_font_stack,
        );
        self.mark_typed_property_evidence(
            evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
            seal.has_visible_font_size,
            false,
        );
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
        has_visible_text: bool,
        has_unverified_consumer: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
        } else if self.terminal_seal.get().is_none() || has_unverified_consumer {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
        } else if !has_visible_text {
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
        expected_text_runs: Vec<JourneyTypographyTextRun<'a>>,
    ) -> Self {
        Self {
            expected_font_family_css,
            expected_font_size_css,
            expected_text_runs: expected_text_runs.into_boxed_slice(),
            next_text_run: 0,
            css_emitted: false,
            css_matches: false,
            outcome: JourneyTypographyTerminalSeal {
                has_visible_text: false,
                has_visible_inherited_font_weight: false,
                has_visible_font_stack: false,
                has_unverified_font_stack: false,
                has_visible_font_size: false,
            },
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

    pub(crate) fn record_text_run(&mut self, text: &str, has_visible_text: bool) {
        let expected = self.expected_text_runs.get(self.next_text_run).copied();
        self.valid &= expected.is_some_and(|run| run.text == text);
        if let Some(run) = expected {
            self.next_text_run = self.next_text_run.saturating_add(1);
            if has_visible_text && run.visibility != Some(false) {
                self.outcome.has_visible_text = true;
                self.outcome.has_visible_inherited_font_weight |= run.inherits_font_weight;
                match run.font_family_ownership {
                    CssFontFamilyOwnership::Inherited => {
                        if run.visibility.is_some() {
                            self.outcome.has_visible_font_stack = true;
                        } else {
                            self.outcome.has_unverified_font_stack = true;
                        }
                    }
                    CssFontFamilyOwnership::Unverified => {
                        self.outcome.has_unverified_font_stack = true;
                    }
                    CssFontFamilyOwnership::SourceOwned => {}
                }
                self.outcome.has_visible_font_size |= run.inherits_font_size;
            }
        }
    }

    fn seal(self) -> Option<JourneyTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.next_text_run == self.expected_text_runs.len())
        .then_some(self.outcome)
    }
}

fn journey_text_runs<'a>(
    layout: &'a JourneyDiagramLayout,
    settings: &JourneyRenderSettings,
    base_font_size_px: f64,
    title: Option<&'a str>,
    title_visibility: Option<bool>,
) -> Vec<JourneyTypographyTextRun<'a>> {
    let mut runs = layout
        .actor_legend
        .iter()
        .flat_map(|item| {
            item.label_lines
                .iter()
                .map(|line| JourneyTypographyTextRun {
                    text: line.text.as_str(),
                    visibility: Some(base_font_size_px > 0.0),
                    font_family_ownership: CssFontFamilyOwnership::Inherited,
                    inherits_font_size: true,
                    inherits_font_weight: true,
                })
        })
        .collect::<Vec<_>>();
    let font_family_ownership = settings.label_font_family_ownership();
    let inherits_font_size = settings.label_inherits_font_size();
    let font_size_px = if inherits_font_size {
        base_font_size_px
    } else {
        settings.task_text_style.font_size
    };
    let mut sections = layout.sections.iter();
    let mut last_section = "";
    for task in &layout.tasks {
        if last_section != task.section
            && let Some(section) = sections.next()
        {
            runs.push(JourneyTypographyTextRun {
                text: section.section.as_str(),
                visibility: Some(font_size_px > 0.0),
                font_family_ownership,
                inherits_font_size,
                inherits_font_weight: true,
            });
        }
        last_section = task.section.as_str();
        runs.push(JourneyTypographyTextRun {
            text: task.task.as_str(),
            visibility: Some(font_size_px > 0.0),
            font_family_ownership,
            inherits_font_size,
            inherits_font_weight: true,
        });
    }
    if let Some(title) = title {
        runs.push(JourneyTypographyTextRun {
            text: title,
            visibility: title_visibility,
            font_family_ownership: settings.title_font_family_ownership(),
            inherits_font_size: settings.title_inherits_font_size(),
            inherits_font_weight: false,
        });
    }
    runs
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
            JourneyTypographyThemePlan::resolve(Some(&theme), &PresentationConfig::empty_object());
        let mut receipt = plan
            .begin_terminal_receipt(&layout, None)
            .expect("theme receipt");
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
            JourneyTypographyThemePlan::resolve(Some(&theme), &PresentationConfig::empty_object());
        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_css(), "18px");
        assert_eq!(plan.font_size_px(), 18.0);
        for size in [0.0, 0.5] {
            let config = crate::test_support::presentation_config(
                serde_json::json!({"themeVariables": {"fontSize": format!("{size}px")}}),
            );
            let plan = JourneyTypographyThemePlan::resolve(Some(&theme), &config);
            assert_eq!(plan.font_size_px(), size);
            assert_eq!(plan.font_size_css(), format!("{size}px"));
            assert_eq!(plan.css.render.task_text_style.font_size, 14.0);
        }
    }
}
