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
    FamilyThemeResidualReason, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug)]
pub(crate) struct TimelineTextPaintPlan {
    fill: Option<DirectStaticPaint>,
    label_fill: Option<DirectStaticPaint>,
    label_owned: Vec<bool>,
    evidence: FamilyThemeOutcomes,
    pending: Vec<TimelinePendingTextMechanism>,
    terminal: OnceLock<TimelineTextPaintOutcome>,
}

#[derive(Debug)]
struct TimelinePendingTextMechanism {
    key: FamilyThemeMechanismKey,
    residual: Option<FamilyThemeResidualReason>,
    inherited_fill_only: bool,
    category_labels: bool,
}

#[derive(Debug)]
pub(crate) struct TimelineTextPaintOutcome {
    inherited_fill_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
    unverified_text_seen: bool,
    unverified_inherited_fill_seen: bool,
    category_label_seen: bool,
    category_fill_seen: bool,
    unverified_category_label_seen: bool,
    unverified_category_fill_seen: bool,
    category_css_matches: bool,
}

impl Default for TimelineTextPaintOutcome {
    fn default() -> Self {
        Self {
            inherited_fill_seen: false,
            text_seen: false,
            source_colors_static: true,
            unverified_text_seen: false,
            unverified_inherited_fill_seen: false,
            category_label_seen: false,
            category_fill_seen: false,
            unverified_category_label_seen: false,
            unverified_category_fill_seen: false,
            category_css_matches: true,
        }
    }
}

impl TimelineTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: None,
            label_fill: None,
            label_owned: Vec::new(),
            evidence: FamilyThemeOutcomes::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let config_owned = super::TimelineCssBinding::config_owns_color(config, "textColor");
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
                    category_labels: false,
                });
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        let mut fallbacks = FamilyThemeOutcomes::default();
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
                category_labels: false,
            });
        }
        plan.resolve_category_labels(theme, config, work)?;
        Ok(plan)
    }

    fn resolve_category_labels(
        &mut self,
        theme: &ResolvedDiagramTheme,
        config: &PresentationConfig,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::NodeLabel,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::NodeLabel
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::NodeLabel,
                    ..
                }
            )
        }) {
            return Ok(());
        }
        let binding = super::TimelineCssBinding::resolve(config.as_value());
        self.label_owned = (0..binding.sections.len())
            .map(|slot| {
                if binding.is_redux_theme {
                    super::TimelineCssBinding::config_owns_color(config, "nodeBorder")
                } else {
                    super::TimelineCssBinding::config_owns_color(
                        config,
                        &format!("cScaleLabel{slot}"),
                    )
                }
            })
            .collect();
        // Redux text follows nodeBorder; gradient Neo has a different surface owner.
        // Keep this extension in the same bounded domain as the category palette.
        let style = theme.style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            None,
            work,
        )?;
        if !binding.is_redux_theme && !binding.use_neo_gradient {
            self.label_fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::NodeLabel],
                DirectStaticSelectorDomain::Default,
            );
        }
        let winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::NodeLabel,
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
            ) && !winners
                .iter()
                .any(|(index, candidate)| *candidate == property && *index > rule_index);
            if !winners.contains(&(rule_index, property)) && !unproved_ordinal {
                continue;
            }
            entry.2 &= property == ResolvedStyleProperty::Fill;
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && self
                    .label_fill
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
                self.pending.push(TimelinePendingTextMechanism {
                    key,
                    residual,
                    inherited_fill_only,
                    category_labels: true,
                });
            } else {
                self.evidence.mark_not_applicable(key);
            }
        }
        let mut fallbacks = FamilyThemeOutcomes::default();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut fallbacks,
            &[UnsupportedTerminalDomain::static_fallbacks_only(
                ThemeTarget::NodeLabel,
                ThemeVariant::Default,
            )],
            work,
        )?;
        for key in fallbacks.not_applicable_mechanisms() {
            self.evidence.mark_not_applicable(key.clone());
        }
        for residual in fallbacks.residuals() {
            self.pending.push(TimelinePendingTextMechanism {
                key: residual.key().clone(),
                residual: Some(residual.reason()),
                inherited_fill_only: residual.reason()
                    == FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                category_labels: true,
            });
        }
        Ok(())
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn label_fill_css(&self, slot: Option<usize>) -> Option<&str> {
        let slot = slot?;
        (!self.label_owned.get(slot).copied().unwrap_or(true))
            .then(|| self.label_fill.as_ref().map(DirectStaticPaint::css))
            .flatten()
    }

    pub(crate) fn record_category_label(
        &self,
        outcome: &mut TimelineTextPaintOutcome,
        slot: Option<usize>,
        emitted_fill: Option<&str>,
        visible: bool,
        font_size_known: bool,
    ) {
        outcome.category_css_matches &= emitted_fill == self.label_fill_css(slot);
        if visible {
            let owned = slot
                .and_then(|slot| self.label_owned.get(slot))
                .copied()
                .unwrap_or(false);
            outcome.category_label_seen = true;
            outcome.category_fill_seen |= !owned;
            outcome.unverified_category_label_seen |= !font_size_known;
            outcome.unverified_category_fill_seen |= !owned && !font_size_known;
        }
    }

    pub(crate) fn begin_emission(&self) -> Option<TimelineTextPaintOutcome> {
        (!self.pending.is_empty()).then(TimelineTextPaintOutcome::default)
    }

    pub(crate) fn record_emission(&self, outcome: TimelineTextPaintOutcome) {
        let _ = self.terminal.set(outcome);
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for pending in &self.pending {
            if pending.category_labels {
                let key = pending.key.clone();
                let Some(outcome) = self.terminal.get() else {
                    evidence.mark_residual(
                        key,
                        pending
                            .residual
                            .unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                    );
                    continue;
                };
                let (seen, unverified) = if pending.inherited_fill_only {
                    (
                        outcome.category_fill_seen,
                        outcome.unverified_category_fill_seen,
                    )
                } else {
                    (
                        outcome.category_label_seen,
                        outcome.unverified_category_label_seen,
                    )
                };
                if !seen {
                    evidence.mark_not_applicable(key);
                } else if !unverified
                    && outcome.category_css_matches
                    && pending.residual.is_none()
                    && let Some(fill) = &self.label_fill
                {
                    evidence.mark_applied_with_capabilities(key, [fill.capability()]);
                } else {
                    evidence.mark_residual(
                        key,
                        pending
                            .residual
                            .unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                    );
                }
                continue;
            }
            match self.terminal.get() {
                Some(outcome)
                    if if pending.inherited_fill_only {
                        outcome.unverified_inherited_fill_seen
                    } else {
                        outcome.unverified_text_seen
                    } =>
                {
                    evidence.mark_residual(
                        pending.key.clone(),
                        pending
                            .residual
                            .unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                    );
                }
                Some(outcome)
                    if !outcome.inherited_fill_seen
                        && if pending.inherited_fill_only {
                            outcome.source_colors_static
                        } else {
                            !outcome.text_seen
                        } =>
                {
                    evidence.mark_not_applicable(pending.key.clone());
                }
                Some(outcome) if outcome.source_colors_static && pending.residual.is_none() => {
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
    }
}

impl TimelineTextPaintOutcome {
    pub(crate) fn record_title(&mut self, inherits_fill: bool, font_size_known: bool) {
        self.text_seen = true;
        self.inherited_fill_seen |= inherits_fill;
        self.unverified_text_seen |= !font_size_known;
        self.unverified_inherited_fill_seen |= inherits_fill && !font_size_known;
    }

    pub(crate) fn record_marker_reference(&mut self) {
        self.inherited_fill_seen = true;
    }

    pub(crate) fn record_node(
        &mut self,
        shape_fill: Option<&str>,
        label_fill: Option<&str>,
        has_visible_label: bool,
        font_size_known: bool,
    ) {
        self.record_fill(shape_fill);
        if has_visible_label {
            self.text_seen = true;
            let inherits_fill = self.record_fill(label_fill);
            self.unverified_text_seen |= !font_size_known;
            self.unverified_inherited_fill_seen |= inherits_fill && !font_size_known;
        }
    }

    fn record_fill(&mut self, fill: Option<&str>) -> bool {
        match fill.map(str::trim) {
            None => {
                self.inherited_fill_seen = true;
                true
            }
            Some(fill)
                if fill.eq_ignore_ascii_case("inherit") || fill.eq_ignore_ascii_case("unset") =>
            {
                self.inherited_fill_seen = true;
                true
            }
            Some(fill)
                if fill.eq_ignore_ascii_case("none") || fill.eq_ignore_ascii_case("initial") =>
            {
                false
            }
            Some(fill) => {
                self.source_colors_static &=
                    crate::mermaid_style::is_supported_css_color_value(fill);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_and_dynamic_colors_have_distinct_evidence() {
        for fill in [None, Some(" Inherit "), Some("UNSET")] {
            let mut outcome = TimelineTextPaintOutcome::default();
            outcome.record_node(fill, Some("#abcdef"), true, true);
            assert!(outcome.inherited_fill_seen);
            assert!(outcome.source_colors_static);
        }
        for fill in ["#abcdef", "transparent", "none", "initial"] {
            let mut outcome = TimelineTextPaintOutcome::default();
            outcome.record_node(Some(fill), Some(fill), true, true);
            assert!(!outcome.inherited_fill_seen);
            assert!(outcome.source_colors_static);
        }
        let mut outcome = TimelineTextPaintOutcome::default();
        outcome.record_node(Some("currentColor"), Some("#abcdef"), true, true);
        assert!(!outcome.source_colors_static);
    }
}
