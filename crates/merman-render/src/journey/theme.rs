use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_TASK_RADIUS_PX: f64 = 3.0;

/// Final Journey task radius shared by terminal SVG emission and family evidence.
#[derive(Debug)]
pub(crate) struct JourneyTaskTheme {
    task_count: usize,
    radius: JourneyTaskRadius,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

#[derive(Debug)]
struct JourneyTaskRadius {
    token: Box<str>,
}

impl JourneyTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        task_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(task_count));
        };

        let static_style = theme.style_with_work_meter(
            ThemeTarget::JourneyTask,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winners = static_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let static_radius_candidate = typed_radius(theme, &static_style);
        let direct_static_winner_rule = static_style
            .radius_resolution()
            .winner()
            .filter(|origin| {
                theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
                    == Some(FamilyThemeDisposition::TypedAdapter)
            })
            .map(|origin| origin.rule_index());

        let has_ordinal_task_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::JourneyTask && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::new();
        let mut static_radius_wins_every_task =
            task_count != 0 && direct_static_winner_rule.is_some();
        if task_count != 0 {
            if has_ordinal_task_rules {
                for ordinal in 1..=task_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::JourneyTask,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    occurrence_winners.extend(
                        style
                            .winner_rule_properties()
                            .into_iter()
                            .map(|(property, origin)| (origin.rule_index(), property)),
                    );
                    static_radius_wins_every_task &= style
                        .radius_resolution()
                        .winner()
                        .map(|origin| origin.rule_index())
                        == direct_static_winner_rule;
                }
            } else {
                occurrence_winners.extend(static_winners.iter().copied());
            }
        }
        let terminal_winner_rule = static_radius_wins_every_task
            .then_some(direct_static_winner_rule)
            .flatten();
        let radius = terminal_winner_rule
            .and(static_radius_candidate)
            .unwrap_or_else(JourneyTaskRadius::baseline);

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, JourneyTaskRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::JourneyTask,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !selector.ordinal_domain_intersects_occurrence_count(task_count) {
                        continue;
                    }
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    let route_won = match selector {
                        FamilyThemeSelectorShape::Static { variant: None } => {
                            static_winners.contains(&(rule_index, property))
                        }
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                        | FamilyThemeSelectorShape::Ordinal { .. } => {
                            occurrence_winners.contains(&(rule_index, property))
                        }
                    };
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Radius,
                        ) if terminal_winner_rule == Some(rule_index) => {
                            observation.radius_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::JourneyTask,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if task_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::JourneyTask,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if task_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let mut pending_radius_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::JourneyTask,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule stays unaccounted until every winning facet has a terminal owner.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            task_count,
            radius,
            evidence,
            pending_radius_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(task_count: usize) -> Self {
        Self {
            task_count,
            radius: JourneyTaskRadius::baseline(),
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) const fn task_count(&self) -> usize {
        self.task_count
    }

    pub(crate) fn radius_token(&self) -> &str {
        &self.radius.token
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<JourneyTaskRadiusThemeReceipt> {
        self.pending_radius_key
            .as_ref()
            .map(|_| JourneyTaskRadiusThemeReceipt::new(self.task_count, self.radius.token.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTaskRadiusThemeReceipt) -> bool {
        self.pending_radius_key.is_some()
            && receipt.proves(self.task_count)
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_radius_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
        }
        evidence
    }
}

impl JourneyTaskRadius {
    fn baseline() -> Self {
        Self {
            token: MERMAID_TASK_RADIUS_PX.to_string().into_boxed_str(),
        }
    }
}

fn typed_radius(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<JourneyTaskRadius> {
    let origin = style.radius_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.radius_resolution().specified() {
        Specified::Value(value) => Some(JourneyTaskRadius {
            token: value.to_string().into_boxed_str(),
        }),
        Specified::Clear => Some(JourneyTaskRadius::baseline()),
        Specified::Unspecified => None,
    }
}

/// Writer-owned proof that every Journey task emitted the resolved terminal radius once.
#[derive(Debug)]
pub(crate) struct JourneyTaskRadiusThemeReceipt {
    expected_task_count: usize,
    expected_radius_token: Box<str>,
    next_task_index: usize,
    attributes_match: bool,
}

impl JourneyTaskRadiusThemeReceipt {
    fn new(expected_task_count: usize, expected_radius_token: Box<str>) -> Self {
        Self {
            expected_task_count,
            expected_radius_token,
            next_task_index: 0,
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_task(
        &mut self,
        task_index: usize,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
    ) {
        if task_index != self.next_task_index {
            self.attributes_match = false;
            return;
        }
        self.next_task_index = self.next_task_index.saturating_add(1);
        self.attributes_match &= emitted_rx_token == self.expected_radius_token.as_ref()
            && emitted_ry_token == self.expected_radius_token.as_ref();
    }

    fn proves(&self, expected_task_count: usize) -> bool {
        self.expected_task_count == expected_task_count
            && self.next_task_index == expected_task_count
            && self.attributes_match
    }
}

#[derive(Debug, Default)]
struct JourneyTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::JourneyTaskRadiusThemeReceipt;

    #[test]
    fn task_radius_receipt_requires_ordered_matching_terminal_rects() {
        let mut complete = JourneyTaskRadiusThemeReceipt::new(1, "9".into());
        complete.record_checkpointed_task(0, "9", "9");
        assert!(complete.proves(1));

        let mut incomplete = JourneyTaskRadiusThemeReceipt::new(2, "9".into());
        incomplete.record_checkpointed_task(0, "9", "9");
        assert!(!incomplete.proves(2));

        let mut mismatch = JourneyTaskRadiusThemeReceipt::new(1, "9".into());
        mismatch.record_checkpointed_task(0, "9", "3");
        assert!(!mismatch.proves(1));
    }
}
