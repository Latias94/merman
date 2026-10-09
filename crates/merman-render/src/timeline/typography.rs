use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_theme_or_root_font_size_px;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};
use crate::model::TimelineDiagramLayout;

/// Resolves Timeline's base typography once for layout, CSS emission, and evidence.
#[derive(Debug)]
pub(crate) struct TimelineTypographyThemePlan {
    css_binding: super::TimelineCssBinding,
    common_css: crate::svg::PreparedCommonCss,
    layout_settings: super::config::TimelineLayoutSettings,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeEvidence,
    terminal_seal: OnceLock<TimelineTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct TimelineTypographyTerminalSeal {
    has_visible_base_text: bool,
}

/// Writer-owned receipt for Timeline's inherited text terminals and root stylesheet.
#[derive(Debug)]
pub(crate) struct TimelineTypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    expected_text_runs: Box<[&'a str]>,
    next_text_run: usize,
    css_emitted: bool,
    css_matches: bool,
    has_visible_base_text: bool,
    valid: bool,
}

impl TimelineTypographyThemePlan {
    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self::resolve_with_text(theme, effective_config, None)
    }

    pub(crate) fn resolve_with_text(
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
        // Root `fontSize` remains Timeline's independent layout-offset owner. Only an explicit
        // theme-variable value shadows the typed base-size route itself.
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let (font_size_px, font_size_css) = match theme {
            Some(theme) if typed_font_size_active => {
                let font_size_px = theme.typography().font_size_px().max(1.0);
                (
                    f64::from(font_size_px),
                    format!("{font_size_px}px").into_boxed_str(),
                )
            }
            _ => {
                let font_size_px =
                    config_theme_or_root_font_size_px(effective_config.as_value(), 16.0).max(1.0);
                (font_size_px, format!("{font_size_px}px").into_boxed_str())
            }
        };

        let mut common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            effective_config.as_value(),
            inherited_font_stack.font_family_css().to_owned(),
            font_size_css.to_string(),
        );
        if let Some(fill) = text_fill {
            common_css = common_css.with_text_color(fill);
        }
        let layout_settings = super::TimelineConfigView::new(effective_config.as_value())
            .layout_settings_with_resolved_typography(
                Some(inherited_font_stack.font_family_css()),
                Some(font_size_px),
            );
        Self {
            css_binding: super::TimelineCssBinding::resolve(effective_config.as_value()),
            common_css,
            layout_settings,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
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

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a TimelineDiagramLayout,
    ) -> Option<TimelineTypographyThemeReceipt<'a>> {
        self.typography_requested().then(|| {
            TimelineTypographyThemeReceipt::new(
                self.font_family_css(),
                self.font_size_css(),
                timeline_text_runs(layout),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: TimelineTypographyThemeReceipt<'_>) -> bool {
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
            return evidence;
        };

        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, seal.has_visible_base_text);
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
            if !seal.has_visible_base_text || !active {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
        }
        evidence
    }
}

impl<'a> TimelineTypographyThemeReceipt<'a> {
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
            && emitted_font_family_css == self.expected_font_family_css;
        self.valid &= emitted_font_size_css == self.expected_font_size_css;
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
    }

    pub(crate) fn record_base_text_run(&mut self, text: &str) {
        self.record_text_run(text);
        self.has_visible_base_text |= !text.trim().is_empty();
    }

    fn seal(self) -> Option<TimelineTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.next_text_run == self.expected_text_runs.len())
        .then_some(TimelineTypographyTerminalSeal {
            has_visible_base_text: self.has_visible_base_text,
        })
    }
}

fn timeline_text_runs(layout: &TimelineDiagramLayout) -> Vec<&str> {
    fn push_node<'a>(runs: &mut Vec<&'a str>, node: &'a crate::model::TimelineNodeLayout) {
        runs.extend(node.label_lines.iter().map(String::as_str));
    }

    let mut runs = Vec::new();
    for section in &layout.sections {
        push_node(&mut runs, &section.node);
        for task in &section.tasks {
            push_node(&mut runs, &task.node);
            for event in &task.events {
                push_node(&mut runs, event);
            }
        }
    }
    for task in &layout.orphan_tasks {
        push_node(&mut runs, &task.node);
        for event in &task.events {
            push_node(&mut runs, event);
        }
    }
    if let Some(title) = layout
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
    {
        runs.push(title);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Bounds, TimelineDiagramLayout, TimelineLineLayout};

    #[test]
    fn prepared_layout_preserves_independent_root_font_offset() {
        let config = MermaidConfig::from_value(serde_json::json!({
            "fontSize": "18px", "themeVariables": {"fontSize": "24px"}
        }));
        let plan =
            TimelineTypographyThemePlan::resolve_with_text(None, &config, Some("var(--text)"));
        assert_eq!(plan.layout_settings().text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        assert_eq!(plan.font_size_css(), "24px");
        assert_eq!(plan.common_css().text_color(), "var(--text)");
        assert_eq!(
            plan.layout_settings().text_style.font_family.as_deref(),
            Some(plan.font_family_css())
        );
    }

    fn empty_layout() -> TimelineDiagramLayout {
        TimelineDiagramLayout {
            direction: merman_core::diagrams::timeline::TimelineDirection::LeftToRight,
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 1.0,
                max_y: 1.0,
            }),
            left_margin: 0.0,
            base_x: 0.0,
            base_y: 0.0,
            pre_title_box_width: 0.0,
            sections: Vec::new(),
            orphan_tasks: Vec::new(),
            activity_line: TimelineLineLayout {
                kind: "activity".to_string(),
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            },
            title: None,
            title_x: 0.0,
            title_y: 0.0,
            use_max_width: false,
        }
    }

    #[test]
    fn empty_typography_receipt_accepts_no_visible_text() {
        let layout = empty_layout();
        let mut receipt =
            TimelineTypographyThemeReceipt::new("Inter", "16px", timeline_text_runs(&layout));
        receipt.record_css_emission("Inter", "16px", true, true);
        let seal = receipt.seal().expect("empty receipt should seal");
        assert!(!seal.has_visible_base_text);
    }

    #[test]
    fn typography_receipt_rejects_wrong_text_order() {
        let mut receipt =
            TimelineTypographyThemeReceipt::new("Inter", "16px", vec!["first", "second"]);
        receipt.record_css_emission("Inter", "16px", true, true);
        receipt.record_text_run("second");
        receipt.record_text_run("first");
        assert!(receipt.seal().is_none());
    }
}
