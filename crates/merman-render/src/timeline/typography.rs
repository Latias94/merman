use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};
use crate::model::TimelineDiagramLayout;

/// Resolves Timeline's inherited font stack once for layout, CSS emission, and evidence.
#[derive(Debug)]
pub(crate) struct TimelineTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    terminal_seal: OnceLock<TimelineTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct TimelineTypographyTerminalSeal {
    has_visible_text: bool,
}

/// Writer-owned receipt for Timeline's inherited text terminals and root stylesheet.
#[derive(Debug)]
pub(crate) struct TimelineTypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_text_runs: Box<[&'a str]>,
    next_text_run: usize,
    css_emitted: bool,
    css_matches: bool,
    has_visible_text: bool,
    valid: bool,
}

impl TimelineTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested()
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a TimelineDiagramLayout,
    ) -> Option<TimelineTypographyThemeReceipt<'a>> {
        self.typography_requested().then(|| {
            TimelineTypographyThemeReceipt::new(self.font_family_css(), timeline_text_runs(layout))
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
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return evidence;
        };

        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, seal.has_visible_text);
        if !self.inherited_font_stack.typed_font_stack_requested() {
            return evidence;
        }

        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if !seal.has_visible_text {
            evidence.mark_not_applicable(key);
        } else if self.inherited_font_stack.typed_font_stack_active() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else {
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }
        evidence
    }
}

impl<'a> TimelineTypographyThemeReceipt<'a> {
    fn new(expected_font_family_css: &'a str, expected_text_runs: Vec<&'a str>) -> Self {
        Self {
            expected_font_family_css,
            expected_text_runs: expected_text_runs.into_boxed_slice(),
            next_text_run: 0,
            css_emitted: false,
            css_matches: false,
            has_visible_text: false,
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(
        &mut self,
        emitted_font_family_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
    ) {
        self.valid &= !self.css_emitted
            && base_typography_emitted
            && root_typography_emitted
            && emitted_font_family_css == self.expected_font_family_css;
        self.css_emitted = true;
        self.css_matches = emitted_font_family_css == self.expected_font_family_css;
    }

    pub(crate) fn record_text_run(&mut self, text: &str) {
        let expected = self.expected_text_runs.get(self.next_text_run).copied();
        self.valid &= expected == Some(text);
        if expected.is_some() {
            self.next_text_run = self.next_text_run.saturating_add(1);
        }
        self.has_visible_text |= !text.trim().is_empty();
    }

    fn seal(self) -> Option<TimelineTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.next_text_run == self.expected_text_runs.len())
        .then_some(TimelineTypographyTerminalSeal {
            has_visible_text: self.has_visible_text,
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
        let mut receipt = TimelineTypographyThemeReceipt::new("Inter", timeline_text_runs(&layout));
        receipt.record_css_emission("Inter", true, true);
        let seal = receipt.seal().expect("empty receipt should seal");
        assert!(!seal.has_visible_text);
    }

    #[test]
    fn typography_receipt_rejects_wrong_text_order() {
        let mut receipt = TimelineTypographyThemeReceipt::new("Inter", vec!["first", "second"]);
        receipt.record_css_emission("Inter", true, true);
        receipt.record_text_run("second");
        receipt.record_text_run("first");
        assert!(receipt.seal().is_none());
    }
}
