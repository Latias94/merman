use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::JourneyDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Journey's textColor owns inherited paint and generic lines, not task shells or actor colors.
#[derive(Debug)]
pub(crate) struct JourneyTextPaintPlan {
    fill: Option<DirectStaticPaint>,
    title_fill: Option<DirectStaticPaint>,
    title: Option<Box<str>>,
    source_title_color: Box<str>,
    evidence: FamilyThemeOutcomes,
    pending: Vec<(FamilyThemeMechanismKey, Option<FamilyThemeResidualReason>)>,
    terminal: OnceLock<()>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum JourneyTextPaintRole {
    Legend,
    Section,
    Task,
    Title,
    Arrowhead,
    Mouth,
}

/// Bounded writer facts; no per-terminal allocation or SVG interpretation is needed.
#[derive(Debug)]
pub(crate) struct JourneyTextPaintReceipt<'a> {
    expected_fill: Option<&'a str>,
    expected_title: Option<&'a str>,
    expected_title_fill: &'a str,
    expected_labels: [usize; 6],
    labels: [usize; 6],
    expected_lines: usize,
    lines: usize,
    css_declarations: usize,
    valid: bool,
}

impl JourneyTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
        diagram_title: Option<&str>,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: None,
            title_fill: None,
            title: diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(Into::into),
            source_title_color: super::config::JourneyConfigView::new(config.as_value())
                .render_settings()
                .title_color
                .into_boxed_str(),
            evidence: FamilyThemeOutcomes::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let title_owned = !plan.source_title_color.is_empty();
        let mut title_fill_winner = None;
        if plan.title.is_some() && !title_owned {
            let style = theme.style_with_work_meter(
                ThemeTarget::Title,
                ThemeVariant::Default,
                Some(1),
                work,
            )?;
            title_fill_winner = style
                .fill_resolution()
                .winner()
                .map(|origin| origin.rule_index());
            plan.title_fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Title],
                DirectStaticSelectorDomain::Default,
            );
        }
        let config_owned = config.config_paint_overrides_typed_default("themeVariables.textColor");
        let style =
            theme.style_with_work_meter(ThemeTarget::Text, ThemeVariant::Default, None, work)?;
        let winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        if !config_owned {
            plan.fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            );
        }
        // Aggregate all facets before certifying a Rule key. A fill does not prove typography.
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text,
                facet,
                selector,
            } = route.mechanism()
            else {
                continue;
            };
            work.charge(1)?;
            let entry = observations
                .entry(theme.family_mechanism_key(*route))
                .or_insert((false, None));
            let property = resolved_style_property_for_facet(facet);
            // There is no logical ordinal ordering for Journey's inherited Text surface.
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !winners.iter().any(|(winner, winner_property)| {
                *winner_property == property && *winner > rule_index
            });
            if (!winners.contains(&(rule_index, property)) && !unproved_ordinal)
                || (property == ResolvedStyleProperty::Fill && config_owned)
            {
                continue;
            }
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && plan
                    .fill
                    .as_ref()
                    .is_some_and(|fill| fill.rule_index() == rule_index)
            {
                entry.0 = true;
            } else {
                entry.1.get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (key, (pending, residual)) in observations {
            if pending || residual.is_some() {
                plan.pending.push((key, residual));
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                UnsupportedTerminalDomain::static_fallbacks_only(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                )
                .with_source_owned_fill(&[config_owned]),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(
                        usize::from(plan.title.is_some()),
                        ThemeVariant::Default,
                    ),
                )
                .with_source_owned_fill(&[title_owned]),
            ],
            work,
        )?;
        for route in theme.family_mechanism_routes() {
            if let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Title,
                rule_index,
                facet: FamilyThemeRuleFacet::Fill(_),
                ..
            } = route.mechanism()
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
            {
                if title_fill_winner != Some(rule_index) {
                    plan.evidence
                        .mark_not_applicable(theme.family_mechanism_key(*route));
                } else if !plan
                    .title_fill
                    .as_ref()
                    .is_some_and(|fill| fill.rule_index() == rule_index)
                {
                    plan.evidence.mark_residual(
                        theme.family_mechanism_key(*route),
                        FamilyThemeResidualReason::UnsupportedPaint,
                    );
                }
            }
        }
        Ok(plan)
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn title_fill_css(&self) -> Option<&str> {
        self.title_fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &JourneyDiagramLayout,
        has_title: bool,
        work: &OperationWorkMeter,
    ) -> Result<Option<JourneyTextPaintReceipt<'a>>, OperationWorkError> {
        if self.pending.is_empty() && self.title_fill.is_none() {
            return Ok(None);
        }
        let mut legend_count = 0;
        for item in &layout.actor_legend {
            work.charge(1)?;
            legend_count += item.label_lines.len();
        }
        let mut mouth_count = 0;
        for task in &layout.tasks {
            work.charge(1)?;
            mouth_count += usize::from(!matches!(
                task.mouth,
                crate::model::JourneyMouthKind::Ambivalent
            ));
        }
        Ok(Some(JourneyTextPaintReceipt {
            expected_fill: self.fill_css(),
            expected_title: self.title.as_deref(),
            expected_title_fill: self.title_fill_css().unwrap_or(&self.source_title_color),
            expected_labels: [
                legend_count,
                layout.sections.len(),
                layout.tasks.len(),
                usize::from(has_title),
                1,
                mouth_count,
            ],
            labels: [0; 6],
            // The activity line exists even in a diagram with no text or tasks.
            expected_lines: layout.tasks.len() + 1,
            lines: 0,
            css_declarations: 0,
            valid: has_title == self.title.is_some(),
        }))
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTextPaintReceipt<'_>) -> bool {
        receipt.is_complete() && self.terminal.set(()).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        if self.terminal.get().is_some()
            && let Some(fill) = &self.title_fill
        {
            evidence.mark_applied_with_capabilities(
                FamilyThemeMechanismKey::Rule {
                    index: fill.rule_index(),
                    target: ThemeTarget::Title,
                },
                [fill.capability()],
            );
        }
        for (key, residual) in &self.pending {
            if let Some(reason) = residual {
                evidence.mark_residual(key.clone(), *reason);
            } else if let Some(fill) = self.fill.as_ref().filter(|_| self.terminal.get().is_some())
            {
                evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
    }
}

impl JourneyTextPaintReceipt<'_> {
    fn is_complete(&self) -> bool {
        self.valid
            && self.css_declarations == 6
            && self.labels == self.expected_labels
            && self.lines == self.expected_lines
    }

    pub(crate) fn record_css(&mut self, emitted: &str) {
        self.valid &= self.expected_fill.is_none_or(|fill| fill == emitted);
        self.css_declarations += 1;
    }

    pub(crate) fn record_label(&mut self, role: JourneyTextPaintRole, inline_fill: Option<&str>) {
        self.valid &= !matches!(role, JourneyTextPaintRole::Title);
        self.valid &=
            inline_fill.is_none_or(|emitted| self.expected_fill.is_none_or(|fill| fill == emitted));
        self.labels[role as usize] += 1;
    }

    pub(crate) fn record_title(&mut self, title: &str, fill: &str) {
        self.valid &= Some(title) == self.expected_title && fill == self.expected_title_fill;
        self.labels[JourneyTextPaintRole::Title as usize] += 1;
    }

    pub(crate) fn record_line(&mut self) {
        self.lines += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_receipt() -> JourneyTextPaintReceipt<'static> {
        JourneyTextPaintReceipt {
            expected_fill: Some("#123456"),
            expected_title: Some("Title"),
            expected_title_fill: "#987654",
            expected_labels: [1; 6],
            labels: [1; 6],
            expected_lines: 2,
            lines: 2,
            css_declarations: 6,
            valid: true,
        }
    }

    #[test]
    fn text_paint_receipt_rejects_missing_or_duplicate_writer_facts() {
        assert!(complete_receipt().is_complete());
        for count in [0, 2] {
            for role in 0..6 {
                let mut receipt = complete_receipt();
                receipt.labels[role] = count;
                assert!(!receipt.is_complete());
            }
        }
        for css_count in [5, 7] {
            let mut receipt = complete_receipt();
            receipt.css_declarations = css_count;
            assert!(!receipt.is_complete());
        }
        for line_count in [1, 3] {
            let mut receipt = complete_receipt();
            receipt.lines = line_count;
            assert!(!receipt.is_complete());
        }
    }

    #[test]
    fn text_paint_receipt_rejects_wrong_css_and_native_label_paint() {
        let mut css = complete_receipt();
        css.css_declarations = 5;
        css.record_css("#654321");
        assert!(!css.is_complete());

        let mut label = complete_receipt();
        label.labels[JourneyTextPaintRole::Task as usize] = 0;
        label.record_label(JourneyTextPaintRole::Task, Some("#654321"));
        assert!(!label.is_complete());
    }

    #[test]
    fn title_receipt_proves_its_own_color_and_text_independently_of_root_paint() {
        for (title, fill, success) in [
            ("Title", "#987654", true),
            ("Title", "#123456", false),
            ("Other", "#987654", false),
        ] {
            let mut receipt = complete_receipt();
            receipt.labels[JourneyTextPaintRole::Title as usize] = 0;
            receipt.record_title(title, fill);
            assert_eq!(receipt.is_complete(), success);
            receipt.record_title(title, fill);
            assert!(!receipt.is_complete());
        }
    }
}
