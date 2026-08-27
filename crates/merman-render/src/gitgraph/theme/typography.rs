use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::{config_f64_css_px, config_string};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};
use crate::model::{GitGraphCommitLayout, GitGraphDiagramLayout};
use crate::text::TextStyle;

const DEFAULT_FONT_SIZE_PX: f64 = 16.0;
const DEFAULT_LABEL_FONT_SIZE_PX: f64 = 10.0;
const TITLE_FONT_SIZE_PX: f64 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitGraphRoleFontSizeDependency {
    Fixed,
    BaseDependent,
    Unmeasurable,
}

#[derive(Debug)]
struct GitGraphRoleFontSize {
    css: Box<str>,
    measured_px: f64,
    dependency: GitGraphRoleFontSizeDependency,
}

impl GitGraphRoleFontSize {
    fn resolve(raw: String, base_font_size_px: f64) -> Self {
        let normalized = normalize_role_font_size(&raw);
        let (measured_px, dependency) = if normalized.eq_ignore_ascii_case("inherit")
            || normalized.eq_ignore_ascii_case("unset")
        {
            (
                base_font_size_px,
                GitGraphRoleFontSizeDependency::BaseDependent,
            )
        } else if let Some(value) = parse_nonnegative_css_unit(normalized, "px") {
            (value, GitGraphRoleFontSizeDependency::Fixed)
        } else if let Some(value) = parse_nonnegative_css_unit(normalized, "em") {
            (
                value * base_font_size_px,
                GitGraphRoleFontSizeDependency::BaseDependent,
            )
        } else if let Some(value) = parse_nonnegative_css_unit(normalized, "%") {
            (
                value * base_font_size_px / 100.0,
                GitGraphRoleFontSizeDependency::BaseDependent,
            )
        } else if normalized
            .parse::<f64>()
            .ok()
            .is_some_and(|value| value.is_finite() && value == 0.0)
        {
            (0.0, GitGraphRoleFontSizeDependency::Fixed)
        } else {
            (
                DEFAULT_LABEL_FONT_SIZE_PX,
                GitGraphRoleFontSizeDependency::Unmeasurable,
            )
        };

        Self {
            css: raw.into_boxed_str(),
            measured_px,
            dependency,
        }
    }

    fn css(&self) -> &str {
        &self.css
    }
}

/// Final GitGraph typography shared by preparation, layout, stylesheet emission, and text probes.
#[derive(Debug)]
pub(crate) struct GitGraphTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    font_size_px: f64,
    commit_label_font_size: GitGraphRoleFontSize,
    tag_label_font_size: GitGraphRoleFontSize,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<GitGraphTypographyTerminalSeal>,
}

impl GitGraphTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let inherited_font_stack = InheritedFontStackPlan::resolve_with_typed_sibling_owners(
            theme,
            effective_config,
            &[ThemeTypographyProperty::FontSize],
        );
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested
            && !config_owns_font_size
            && inherited_font_stack.outcome() != InheritedFontStackOutcome::Unsupported;
        let font_size_px = match theme {
            Some(theme) if typed_font_size_active => f64::from(theme.typography().font_size_px()),
            _ => config_f64_css_px(effective_config.as_value(), &["themeVariables", "fontSize"])
                .unwrap_or(DEFAULT_FONT_SIZE_PX),
        }
        .max(1.0);
        let font_size_css = format!("{font_size_px}px").into_boxed_str();
        let commit_label_font_size = GitGraphRoleFontSize::resolve(
            config_string(
                effective_config.as_value(),
                &["themeVariables", "commitLabelFontSize"],
            )
            .unwrap_or_else(|| "10px".to_string()),
            font_size_px,
        );
        let tag_label_font_size = GitGraphRoleFontSize::resolve(
            config_string(
                effective_config.as_value(),
                &["themeVariables", "tagLabelFontSize"],
            )
            .unwrap_or_else(|| "10px".to_string()),
            font_size_px,
        );

        Self {
            inherited_font_stack,
            font_size_css,
            font_size_px,
            commit_label_font_size,
            tag_label_font_size,
            typed_font_size_requested,
            typed_font_size_active,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn branch_label_style(&self) -> TextStyle {
        self.text_style(self.font_size_px)
    }

    pub(crate) fn commit_label_font_size_css(&self) -> &str {
        self.commit_label_font_size.css()
    }

    pub(crate) fn commit_label_style(&self) -> TextStyle {
        self.text_style(self.commit_label_font_size.measured_px)
    }

    pub(crate) fn tag_label_font_size_css(&self) -> &str {
        self.tag_label_font_size.css()
    }

    pub(crate) fn tag_label_style(&self) -> TextStyle {
        self.text_style(self.tag_label_font_size.measured_px)
    }

    pub(crate) fn title_style(&self) -> TextStyle {
        self.text_style(TITLE_FONT_SIZE_PX)
    }

    pub(crate) const fn title_font_size_px(&self) -> f64 {
        TITLE_FONT_SIZE_PX
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a GitGraphDiagramLayout,
        title: Option<&'a str>,
    ) -> Option<GitGraphTypographyThemeReceipt<'a>> {
        self.typography_requested()
            .then(|| GitGraphTypographyThemeReceipt::new(self, layout, title))
    }

    pub(crate) fn record_terminal(&self, receipt: GitGraphTypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_receipt.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.typography_requested() {
            return evidence;
        }

        let key = FamilyThemeMechanismKey::Typography;
        if self.inherited_font_stack.outcome() == InheritedFontStackOutcome::Unsupported {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            return evidence;
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            return evidence;
        };

        let typed_typography_active =
            self.inherited_font_stack.typed_font_stack_active() || self.typed_font_size_active;
        if typed_typography_active && receipt.has_visible_unmeasurable_role_font_size_terminal {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            return evidence;
        }

        let font_stack_applied = self.inherited_font_stack.typed_font_stack_active()
            && receipt.has_visible_font_stack_terminal;
        let font_size_applied = self.typed_font_size_active
            && (receipt.has_visible_branch_label
                || receipt.has_visible_base_dependent_role_font_size_terminal);
        if font_stack_applied || font_size_applied {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else {
            evidence.mark_not_applicable(key);
        }
        evidence
    }

    fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typed_font_stack_requested()
            || self.typed_font_size_requested
            || self.inherited_font_stack.outcome() == InheritedFontStackOutcome::Unsupported
    }

    fn text_style(&self, font_size_px: f64) -> TextStyle {
        TextStyle {
            font_family: Some(self.font_family_css().to_string()),
            font_size: font_size_px,
            font_weight: None,
            font_style: None,
        }
    }
}

pub(crate) fn gitgraph_commit_label_is_visible(
    layout: &GitGraphDiagramLayout,
    commit: &GitGraphCommitLayout,
) -> bool {
    (commit.commit_type != 3 || commit.custom_id.unwrap_or(false))
        && commit.commit_type != 4
        && layout.show_commit_label
}

pub(crate) fn gitgraph_tags_in_output_order(
    commit: &GitGraphCommitLayout,
) -> impl ExactSizeIterator<Item = &str> + Clone {
    commit.tags.iter().rev().map(String::as_str)
}

fn normalize_role_font_size(raw: &str) -> &str {
    let raw = raw.trim().trim_end_matches(';').trim();
    raw.strip_suffix("!important").unwrap_or(raw).trim()
}

fn parse_nonnegative_css_unit(raw: &str, unit: &str) -> Option<f64> {
    let value = raw.strip_suffix(unit)?.trim().parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

/// Exact base typography values observed after the GitGraph stylesheet reached the SVG sink.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GitGraphTypographyCssEmission<'a> {
    pub(crate) base_font_family_css: &'a str,
    pub(crate) branch_label_font_family_css: &'a str,
    pub(crate) base_font_size_css: &'a str,
    pub(crate) base_css_emitted: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum ExpectedTextEvent {
    Branch {
        index: usize,
    },
    Commit {
        index: usize,
    },
    Tag {
        commit_index: usize,
        output_index: usize,
    },
    Title,
    Complete,
}

/// Writer-owned proof that the exact stylesheet and ordered visible text passes were emitted.
#[derive(Debug)]
pub(crate) struct GitGraphTypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    layout: &'a GitGraphDiagramLayout,
    title: Option<&'a str>,
    next_event: ExpectedTextEvent,
    css_emitted: bool,
    has_visible_font_stack_terminal: bool,
    has_visible_branch_label: bool,
    has_visible_base_dependent_role_font_size_terminal: bool,
    has_visible_unmeasurable_role_font_size_terminal: bool,
    commit_label_font_size_dependency: GitGraphRoleFontSizeDependency,
    tag_label_font_size_dependency: GitGraphRoleFontSizeDependency,
    valid: bool,
}

#[derive(Debug)]
struct GitGraphTypographyTerminalSeal {
    has_visible_font_stack_terminal: bool,
    has_visible_branch_label: bool,
    has_visible_base_dependent_role_font_size_terminal: bool,
    has_visible_unmeasurable_role_font_size_terminal: bool,
}

impl<'a> GitGraphTypographyThemeReceipt<'a> {
    fn new(
        plan: &'a GitGraphTypographyThemePlan,
        layout: &'a GitGraphDiagramLayout,
        title: Option<&'a str>,
    ) -> Self {
        let next_event = if layout.show_branches && !layout.branches.is_empty() {
            ExpectedTextEvent::Branch { index: 0 }
        } else {
            Self::next_commit_event(layout, 0)
        };

        Self {
            expected_font_family_css: plan.font_family_css(),
            expected_font_size_css: plan.font_size_css(),
            layout,
            title,
            next_event,
            css_emitted: false,
            has_visible_font_stack_terminal: false,
            has_visible_branch_label: false,
            has_visible_base_dependent_role_font_size_terminal: false,
            has_visible_unmeasurable_role_font_size_terminal: false,
            commit_label_font_size_dependency: plan.commit_label_font_size.dependency,
            tag_label_font_size_dependency: plan.tag_label_font_size.dependency,
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(&mut self, emission: GitGraphTypographyCssEmission<'_>) {
        self.valid &= !self.css_emitted
            && emission.base_css_emitted
            && emission.base_font_family_css == self.expected_font_family_css
            && emission.branch_label_font_family_css == self.expected_font_family_css
            && emission.base_font_size_css == self.expected_font_size_css;
        self.css_emitted = true;
    }

    pub(crate) fn record_branch_label(&mut self, index: usize, text: &str) {
        let matched = matches!(
            self.next_event,
            ExpectedTextEvent::Branch { index: expected_index }
                if expected_index == index
                    && self.layout.branches.get(index).map(|branch| branch.name.as_str())
                        == Some(text)
        );
        self.record_text_event(matched, !text.trim().is_empty(), true);
    }

    pub(crate) fn record_commit_label(&mut self, index: usize, text: &str) {
        let matched = matches!(
            self.next_event,
            ExpectedTextEvent::Commit { index: expected_index }
                if expected_index == index
                    && self.layout.commits.get(index).map(|commit| commit.id.as_str()) == Some(text)
        );
        let visible = !text.trim().is_empty();
        self.record_text_event(matched, visible, false);
        if matched && visible {
            self.record_role_font_size(self.commit_label_font_size_dependency);
        }
    }

    pub(crate) fn record_tag_label(
        &mut self,
        commit_index: usize,
        output_index: usize,
        text: &str,
    ) {
        let matched = matches!(
            self.next_event,
            ExpectedTextEvent::Tag {
                commit_index: expected_commit_index,
                output_index: expected_output_index,
            } if expected_commit_index == commit_index
                && expected_output_index == output_index
                && self.expected_tag_text(commit_index, output_index) == Some(text)
        );
        let visible = !text.trim().is_empty();
        self.record_text_event(matched, visible, false);
        if matched && visible {
            self.record_role_font_size(self.tag_label_font_size_dependency);
        }
    }

    pub(crate) fn record_title(&mut self, title: Option<&str>) {
        let title = title.map(str::trim).filter(|title| !title.is_empty());
        let matched = matches!(
            self.next_event,
            ExpectedTextEvent::Title if self.expected_title() == title
        );
        self.record_text_event(matched, title.is_some(), false);
    }

    fn record_text_event(&mut self, matched: bool, visible: bool, branch: bool) {
        self.valid &= self.css_emitted && matched;
        if matched {
            self.has_visible_font_stack_terminal |= visible;
            self.has_visible_branch_label |= branch && visible;
        }
        self.advance_event();
    }

    fn record_role_font_size(&mut self, dependency: GitGraphRoleFontSizeDependency) {
        match dependency {
            GitGraphRoleFontSizeDependency::Fixed => {}
            GitGraphRoleFontSizeDependency::BaseDependent => {
                self.has_visible_base_dependent_role_font_size_terminal = true;
            }
            GitGraphRoleFontSizeDependency::Unmeasurable => {
                self.has_visible_unmeasurable_role_font_size_terminal = true;
            }
        }
    }

    fn next_commit_event(
        layout: &GitGraphDiagramLayout,
        mut commit_index: usize,
    ) -> ExpectedTextEvent {
        while let Some(commit) = layout.commits.get(commit_index) {
            if gitgraph_commit_label_is_visible(layout, commit) {
                return ExpectedTextEvent::Commit {
                    index: commit_index,
                };
            }
            if !commit.tags.is_empty() {
                return ExpectedTextEvent::Tag {
                    commit_index,
                    output_index: 0,
                };
            }
            commit_index = commit_index.saturating_add(1);
        }
        ExpectedTextEvent::Title
    }

    fn advance_event(&mut self) {
        self.next_event = match self.next_event {
            ExpectedTextEvent::Branch { index }
                if self.layout.show_branches && index + 1 < self.layout.branches.len() =>
            {
                ExpectedTextEvent::Branch { index: index + 1 }
            }
            ExpectedTextEvent::Branch { .. } => Self::next_commit_event(self.layout, 0),
            ExpectedTextEvent::Commit { index } => {
                if self
                    .layout
                    .commits
                    .get(index)
                    .is_some_and(|commit| !commit.tags.is_empty())
                {
                    ExpectedTextEvent::Tag {
                        commit_index: index,
                        output_index: 0,
                    }
                } else {
                    Self::next_commit_event(self.layout, index.saturating_add(1))
                }
            }
            ExpectedTextEvent::Tag {
                commit_index,
                output_index,
            } => {
                let next_output_index = output_index.saturating_add(1);
                if self
                    .layout
                    .commits
                    .get(commit_index)
                    .is_some_and(|commit| next_output_index < commit.tags.len())
                {
                    ExpectedTextEvent::Tag {
                        commit_index,
                        output_index: next_output_index,
                    }
                } else {
                    Self::next_commit_event(self.layout, commit_index.saturating_add(1))
                }
            }
            ExpectedTextEvent::Title | ExpectedTextEvent::Complete => ExpectedTextEvent::Complete,
        };
    }

    fn expected_tag_text(&self, commit_index: usize, output_index: usize) -> Option<&str> {
        gitgraph_tags_in_output_order(self.layout.commits.get(commit_index)?).nth(output_index)
    }

    fn expected_title(&self) -> Option<&str> {
        self.title.map(str::trim).filter(|title| !title.is_empty())
    }

    fn seal(self) -> Option<GitGraphTypographyTerminalSeal> {
        (self.valid && self.css_emitted && self.next_event == ExpectedTextEvent::Complete)
            .then_some(GitGraphTypographyTerminalSeal {
                has_visible_font_stack_terminal: self.has_visible_font_stack_terminal,
                has_visible_branch_label: self.has_visible_branch_label,
                has_visible_base_dependent_role_font_size_terminal: self
                    .has_visible_base_dependent_role_font_size_terminal,
                has_visible_unmeasurable_role_font_size_terminal: self
                    .has_visible_unmeasurable_role_font_size_terminal,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use crate::model::{GitGraphBranchLayout, GitGraphCommitLayout};
    use serde_json::json;

    fn typography_plan() -> GitGraphTypographyThemePlan {
        typography_plan_with_config(MermaidConfig::default())
    }

    fn typography_plan_with_config(config: MermaidConfig) -> GitGraphTypographyThemePlan {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("GitGraphReceipt").expect("valid font stack"))
            .with_font_size_px(23.0)
            .expect("valid font size");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::GIT_GRAPH, typography),
            ))
            .expect("compile GitGraph typography receipt theme")
            .resolve(DiagramFamilyId::GIT_GRAPH);
        GitGraphTypographyThemePlan::resolve(Some(&theme), &config)
    }

    fn layout() -> GitGraphDiagramLayout {
        GitGraphDiagramLayout {
            bounds: None,
            direction: "LR".to_string(),
            rotate_commit_label: false,
            show_branches: true,
            show_commit_label: true,
            parallel_commits: false,
            diagram_padding: 0.0,
            max_pos: 0.0,
            branches: vec![GitGraphBranchLayout {
                name: "main".to_string(),
                index: 0,
                pos: 0.0,
                bbox_width: 0.0,
                bbox_height: 0.0,
            }],
            commits: vec![GitGraphCommitLayout {
                id: "1".to_string(),
                message: "one".to_string(),
                seq: 0,
                commit_type: 0,
                tags: vec!["first".to_string(), "second".to_string()],
                parents: Vec::new(),
                branch: "main".to_string(),
                custom_type: None,
                custom_id: Some(true),
                x: 0.0,
                y: 0.0,
                pos: 0.0,
                pos_with_offset: 0.0,
            }],
            arrows: Vec::new(),
        }
    }

    fn emission<'a>(family: &'a str, size: &'a str) -> GitGraphTypographyCssEmission<'a> {
        GitGraphTypographyCssEmission {
            base_font_family_css: family,
            branch_label_font_family_css: family,
            base_font_size_css: size,
            base_css_emitted: true,
        }
    }

    fn complete_receipt<'a>(
        plan: &'a GitGraphTypographyThemePlan,
        layout: &'a GitGraphDiagramLayout,
    ) -> GitGraphTypographyThemeReceipt<'a> {
        let mut receipt = plan
            .begin_terminal_receipt(layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_branch_label(0, "main");
        receipt.record_commit_label(0, "1");
        receipt.record_tag_label(0, 0, "second");
        receipt.record_tag_label(0, 1, "first");
        receipt.record_title(Some("Graph title"));
        receipt
    }

    #[test]
    fn receipt_accepts_the_complete_writer_order() {
        let plan = typography_plan();
        let layout = layout();
        assert!(plan.record_terminal(complete_receipt(&plan, &layout)));
        assert_eq!(plan.finish_evidence().applied().len(), 1);
    }

    #[test]
    fn receipt_rejects_missing_duplicate_reordered_extra_and_wrong_emission() {
        let missing_css = typography_plan();
        let missing_css_layout = layout();
        let mut receipt = missing_css
            .begin_terminal_receipt(&missing_css_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_branch_label(0, "main");
        assert!(!missing_css.record_terminal(receipt));

        let duplicate_css = typography_plan();
        let duplicate_css_layout = layout();
        let mut receipt = duplicate_css
            .begin_terminal_receipt(&duplicate_css_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        assert!(!duplicate_css.record_terminal(receipt));

        let reordered = typography_plan();
        let reordered_layout = layout();
        let mut receipt = reordered
            .begin_terminal_receipt(&reordered_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_commit_label(0, "1");
        assert!(!reordered.record_terminal(receipt));

        let wrong_tag_order = typography_plan();
        let wrong_tag_order_layout = layout();
        let mut receipt = wrong_tag_order
            .begin_terminal_receipt(&wrong_tag_order_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_branch_label(0, "main");
        receipt.record_commit_label(0, "1");
        receipt.record_tag_label(0, 0, "first");
        assert!(!wrong_tag_order.record_terminal(receipt));

        let missing_title = typography_plan();
        let missing_title_layout = layout();
        let mut receipt = missing_title
            .begin_terminal_receipt(&missing_title_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_branch_label(0, "main");
        receipt.record_commit_label(0, "1");
        receipt.record_tag_label(0, 0, "second");
        receipt.record_tag_label(0, 1, "first");
        assert!(!missing_title.record_terminal(receipt));

        let extra = typography_plan();
        let extra_layout = layout();
        let mut receipt = complete_receipt(&extra, &extra_layout);
        receipt.record_title(Some("Graph title"));
        assert!(!extra.record_terminal(receipt));

        let wrong_emission = typography_plan();
        let wrong_emission_layout = layout();
        let mut receipt = wrong_emission
            .begin_terminal_receipt(&wrong_emission_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("WrongFamily", "22px"));
        assert!(!wrong_emission.record_terminal(receipt));
    }

    #[test]
    fn role_font_sizes_distinguish_fixed_base_dependent_and_unmeasurable_values() {
        for (raw, expected_px, expected_dependency) in [
            ("12px", 12.0, GitGraphRoleFontSizeDependency::Fixed),
            ("0", 0.0, GitGraphRoleFontSizeDependency::Fixed),
            (
                "inherit",
                20.0,
                GitGraphRoleFontSizeDependency::BaseDependent,
            ),
            ("unset", 20.0, GitGraphRoleFontSizeDependency::BaseDependent),
            ("1.5em", 30.0, GitGraphRoleFontSizeDependency::BaseDependent),
            ("125%", 25.0, GitGraphRoleFontSizeDependency::BaseDependent),
            (
                "calc(10px + 1em)",
                DEFAULT_LABEL_FONT_SIZE_PX,
                GitGraphRoleFontSizeDependency::Unmeasurable,
            ),
            (
                "1rem",
                DEFAULT_LABEL_FONT_SIZE_PX,
                GitGraphRoleFontSizeDependency::Unmeasurable,
            ),
        ] {
            let resolved = GitGraphRoleFontSize::resolve(raw.to_string(), 20.0);
            assert_eq!(resolved.css(), raw);
            assert_eq!(resolved.measured_px, expected_px, "raw={raw}");
            assert_eq!(resolved.dependency, expected_dependency, "raw={raw}");
        }
    }

    #[test]
    fn receipt_accounts_for_base_dependent_and_unmeasurable_role_sizes() {
        let inherited = typography_plan_with_config(MermaidConfig::from_value(json!({
            "themeVariables": {
                "commitLabelFontSize": "inherit",
                "tagLabelFontSize": "150%"
            }
        })));
        let mut inherited_layout = layout();
        inherited_layout.show_branches = false;
        let mut receipt = inherited
            .begin_terminal_receipt(&inherited_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_commit_label(0, "1");
        receipt.record_tag_label(0, 0, "second");
        receipt.record_tag_label(0, 1, "first");
        receipt.record_title(Some("Graph title"));
        assert!(inherited.record_terminal(receipt));
        let inherited_seal = inherited.terminal_receipt.get().expect("terminal seal");
        assert!(inherited_seal.has_visible_base_dependent_role_font_size_terminal);
        assert!(!inherited_seal.has_visible_unmeasurable_role_font_size_terminal);
        assert_eq!(inherited.finish_evidence().applied().len(), 1);

        let unmeasurable = typography_plan_with_config(MermaidConfig::from_value(json!({
            "themeVariables": {
                "commitLabelFontSize": "calc(10px + 1em)"
            }
        })));
        let mut unmeasurable_layout = layout();
        unmeasurable_layout.show_branches = false;
        let mut receipt = unmeasurable
            .begin_terminal_receipt(&unmeasurable_layout, Some("Graph title"))
            .expect("typed receipt");
        receipt.record_css_emission(emission("GitGraphReceipt", "23px"));
        receipt.record_commit_label(0, "1");
        receipt.record_tag_label(0, 0, "second");
        receipt.record_tag_label(0, 1, "first");
        receipt.record_title(Some("Graph title"));
        assert!(unmeasurable.record_terminal(receipt));
        let unmeasurable_seal = unmeasurable.terminal_receipt.get().expect("terminal seal");
        assert!(unmeasurable_seal.has_visible_unmeasurable_role_font_size_terminal);
        assert_eq!(unmeasurable.finish_evidence().residuals().len(), 1);
    }
}
