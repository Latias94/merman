use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::TimelineDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug)]
pub(crate) struct TimelineTextPaintPlan {
    fill: Option<DirectStaticPaint>,
    evidence: FamilyThemeEvidence,
    pending: Vec<TimelinePendingTextMechanism>,
    terminal: OnceLock<TimelineTextPaintTerminalSeal>,
}

#[derive(Debug)]
struct TimelinePendingTextMechanism {
    key: FamilyThemeMechanismKey,
    residual: Option<FamilyThemeResidualReason>,
    inherited_fill_only: bool,
}

#[derive(Debug)]
struct TimelineTextPaintTerminalSeal {
    inherited_fill_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
}

#[derive(Debug)]
pub(crate) struct TimelineTextPaintReceipt<'a> {
    expected_fill: Option<&'a str>,
    expected_title: Option<&'a str>,
    expected_nodes: usize,
    nodes: usize,
    css_seen: bool,
    title_seen: bool,
    marker_seen: bool,
    marker_referenced: bool,
    inherited_fill_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
    valid: bool,
}

impl TimelineTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: None,
            evidence: FamilyThemeEvidence::from_theme(theme),
            pending: Vec::new(),
            terminal: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.textColor",
        );
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
                .or_insert((false, None, true));
            let property = resolved_style_property_for_facet(facet);
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !winners.iter().any(|(winner_index, winner_property)| {
                *winner_property == property && *winner_index > rule_index
            });
            if (!winners.contains(&(rule_index, property)) && !unproved_ordinal)
                || (property == ResolvedStyleProperty::Fill && config_owned)
            {
                continue;
            }
            entry.2 &= property == ResolvedStyleProperty::Fill;
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
        for (key, (pending, residual, inherited_fill_only)) in observations {
            if pending || residual.is_some() {
                plan.pending.push(TimelinePendingTextMechanism {
                    key,
                    residual,
                    inherited_fill_only,
                });
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        let mut fallbacks = FamilyThemeEvidence::from_theme(Some(theme));
        reconcile_unsupported_terminal_domains(
            theme,
            &mut fallbacks,
            &[UnsupportedTerminalDomain::static_fallbacks_only(
                ThemeTarget::Text,
                ThemeVariant::Default,
            )
            .with_source_owned_fill(&[config_owned])],
            work,
        )?;
        for key in fallbacks.not_applicable_mechanisms() {
            plan.evidence.mark_not_applicable(key.clone());
        }
        for residual in fallbacks.residuals() {
            plan.pending.push(TimelinePendingTextMechanism {
                key: residual.key().clone(),
                residual: Some(residual.reason()),
                inherited_fill_only: residual.reason()
                    == FamilyThemeResidualReason::UnsupportedOrdinalPalette,
            });
        }
        Ok(plan)
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a TimelineDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Option<TimelineTextPaintReceipt<'a>>, OperationWorkError> {
        if self.pending.is_empty() {
            return Ok(None);
        }
        let mut expected_nodes = layout.sections.len();
        for task in layout
            .sections
            .iter()
            .flat_map(|section| &section.tasks)
            .chain(&layout.orphan_tasks)
        {
            work.charge(1)?;
            expected_nodes += 1 + task.events.len();
        }
        Ok(Some(TimelineTextPaintReceipt {
            expected_fill: self.fill_css(),
            expected_title: layout
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty()),
            expected_nodes,
            nodes: 0,
            css_seen: false,
            title_seen: false,
            marker_seen: false,
            marker_referenced: false,
            inherited_fill_seen: false,
            text_seen: false,
            source_colors_static: true,
            valid: true,
        }))
    }

    pub(crate) fn record_terminal(&self, receipt: TimelineTextPaintReceipt<'_>) -> bool {
        receipt.is_complete()
            && self
                .terminal
                .set(TimelineTextPaintTerminalSeal {
                    inherited_fill_seen: receipt.inherited_fill_seen || receipt.marker_referenced,
                    text_seen: receipt.text_seen,
                    source_colors_static: receipt.source_colors_static,
                })
                .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for pending in &self.pending {
            match self.terminal.get() {
                Some(receipt)
                    if (pending.inherited_fill_only
                        && receipt.source_colors_static
                        && !receipt.inherited_fill_seen)
                        || (!pending.inherited_fill_only
                            && !receipt.text_seen
                            && !receipt.inherited_fill_seen) =>
                {
                    evidence.mark_not_applicable(pending.key.clone());
                }
                Some(receipt) if receipt.source_colors_static && pending.residual.is_none() => {
                    if let Some(fill) = &self.fill {
                        evidence.mark_applied_with_capabilities(
                            pending.key.clone(),
                            [fill.capability()],
                        );
                    } else {
                        evidence.mark_residual(
                            pending.key.clone(),
                            FamilyThemeResidualReason::UnsupportedPaint,
                        );
                    }
                }
                _ => evidence.mark_residual(
                    pending.key.clone(),
                    pending
                        .residual
                        .unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                ),
            }
        }
        evidence
    }
}

impl TimelineTextPaintReceipt<'_> {
    fn is_complete(&self) -> bool {
        self.valid
            && self.css_seen
            && self.marker_seen
            && self.nodes == self.expected_nodes
            && self.title_seen == self.expected_title.is_some()
    }

    pub(crate) fn record_css(&mut self, fill: &str) {
        self.valid &= !self.css_seen && self.expected_fill.is_none_or(|expected| expected == fill);
        self.css_seen = true;
    }

    pub(crate) fn record_title(&mut self, title: &str) {
        self.valid &= !self.title_seen && self.expected_title == Some(title);
        self.title_seen = true;
        self.text_seen = true;
        self.inherited_fill_seen = true;
    }

    pub(crate) fn record_marker(&mut self) {
        self.valid &= !self.marker_seen;
        self.marker_seen = true;
    }

    pub(crate) fn record_marker_reference(&mut self, reference_matches: bool) {
        self.marker_referenced |= reference_matches;
    }

    pub(crate) fn record_node(
        &mut self,
        shape_fill: Option<&str>,
        label_fill: Option<&str>,
        has_visible_label: bool,
    ) {
        self.nodes += 1;
        self.record_fill(shape_fill);
        if has_visible_label {
            self.text_seen = true;
            self.record_fill(label_fill);
        }
    }

    fn record_fill(&mut self, fill: Option<&str>) {
        match fill.map(str::trim) {
            None => self.inherited_fill_seen = true,
            Some(fill)
                if fill.eq_ignore_ascii_case("inherit") || fill.eq_ignore_ascii_case("unset") =>
            {
                self.inherited_fill_seen = true;
            }
            Some(fill)
                if fill.eq_ignore_ascii_case("none") || fill.eq_ignore_ascii_case("initial") => {}
            Some(fill) => {
                self.source_colors_static &=
                    crate::mermaid_style::is_supported_css_color_value(fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt() -> TimelineTextPaintReceipt<'static> {
        TimelineTextPaintReceipt {
            expected_fill: Some("#123456"),
            expected_title: Some("Title"),
            expected_nodes: 1,
            nodes: 0,
            css_seen: false,
            title_seen: false,
            marker_seen: false,
            marker_referenced: false,
            inherited_fill_seen: false,
            text_seen: false,
            source_colors_static: true,
            valid: true,
        }
    }

    fn complete_receipt() -> TimelineTextPaintReceipt<'static> {
        let mut receipt = receipt();
        receipt.record_css("#123456");
        receipt.record_title("Title");
        receipt.record_marker();
        receipt.record_node(Some("#abcdef"), Some("#fedcba"), true);
        receipt
    }

    #[test]
    fn text_receipt_rejects_missing_duplicate_and_mismatched_writer_facts() {
        assert!(complete_receipt().is_complete());
        assert!(!receipt().is_complete());
        for nodes in [0, 2] {
            let mut receipt = complete_receipt();
            receipt.nodes = nodes;
            assert!(!receipt.is_complete());
        }
        let mut wrong_css = receipt();
        wrong_css.record_css("#654321");
        assert!(!wrong_css.valid);
        let mut wrong_title = receipt();
        wrong_title.record_title("Other");
        assert!(!wrong_title.valid);
        let mut duplicate_css = complete_receipt();
        duplicate_css.record_css("#123456");
        assert!(!duplicate_css.is_complete());
        let mut duplicate_title = complete_receipt();
        duplicate_title.record_title("Title");
        assert!(!duplicate_title.is_complete());
        let mut duplicate_marker = complete_receipt();
        duplicate_marker.record_marker();
        assert!(!duplicate_marker.is_complete());
    }

    #[test]
    fn unused_marker_does_not_supply_inherited_fill_evidence() {
        let mut receipt = receipt();
        receipt.record_marker();
        receipt.record_marker_reference(false);
        assert!(!receipt.marker_referenced);
        assert!(!receipt.inherited_fill_seen);
        receipt.record_marker_reference(true);
        assert!(receipt.marker_referenced);
    }

    #[test]
    fn inherited_and_dynamic_colors_have_distinct_evidence() {
        for fill in [None, Some(" Inherit "), Some("UNSET")] {
            let mut receipt = receipt();
            receipt.record_node(fill, Some("#abcdef"), true);
            assert!(receipt.inherited_fill_seen);
            assert!(receipt.source_colors_static);
        }
        for fill in ["#abcdef", "transparent", "none", "initial"] {
            let mut receipt = receipt();
            receipt.record_node(Some(fill), Some(fill), true);
            assert!(!receipt.inherited_fill_seen);
            assert!(receipt.source_colors_static);
        }
        let mut receipt = receipt();
        receipt.record_node(Some("currentColor"), Some("#abcdef"), true);
        assert!(!receipt.source_colors_static);
    }
}
