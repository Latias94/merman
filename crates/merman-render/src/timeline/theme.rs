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
use crate::model::TimelineDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Timeline event opacity and evidence resolved in the exact terminal draw order.
#[derive(Debug)]
pub(crate) struct TimelineEventTheme {
    event_opacity_tokens: Vec<Option<Box<str>>>,
    evidence: FamilyThemeEvidence,
    pending_opacity_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl TimelineEventTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        layout: &TimelineDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let event_count = timeline_event_count(layout);

        let mut event_opacity_tokens = Vec::with_capacity(event_count);
        let mut winner_properties = BTreeSet::new();
        let has_ordinal_event_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::TimelineEvent && rule.ordinal().is_some()
        });
        if event_count != 0 && !has_ordinal_event_rules {
            let style = theme.style_with_work_meter(
                ThemeTarget::TimelineEvent,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            winner_properties.extend(
                style
                    .winner_rule_properties()
                    .into_iter()
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
            event_opacity_tokens.resize(event_count, typed_opacity_token(theme, &style));
        } else {
            for event_ordinal in 1..=event_count {
                let style = theme.style_with_work_meter(
                    ThemeTarget::TimelineEvent,
                    ThemeVariant::Default,
                    Some(event_ordinal),
                    work_meter,
                )?;
                winner_properties.extend(
                    style
                        .winner_rule_properties()
                        .into_iter()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
                event_opacity_tokens.push(typed_opacity_token(theme, &style));
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, TimelineEventRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::TimelineEvent,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if event_count == 0 || (!route_won && !qualified_variant) {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Opacity) => {
                            observation.opacity_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::TimelineEvent,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if event_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::TimelineEvent,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if event_count == 0 {
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

        let mut pending_opacity_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::TimelineEvent,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule cannot be signed Applied until every winning facet is accounted.
            } else if observation.opacity_pending {
                debug_assert!(pending_opacity_key.is_none());
                pending_opacity_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            event_opacity_tokens,
            evidence,
            pending_opacity_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            event_opacity_tokens: Vec::new(),
            evidence: FamilyThemeEvidence::default(),
            pending_opacity_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn opacity_token_for_event(&self, event_index: usize) -> Option<&str> {
        self.event_opacity_tokens
            .get(event_index)
            .and_then(|token| token.as_deref())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<TimelineEventOpacityThemeReceipt> {
        self.pending_opacity_key
            .as_ref()
            .map(|_| TimelineEventOpacityThemeReceipt::new(self.event_opacity_tokens.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: TimelineEventOpacityThemeReceipt) -> bool {
        self.pending_opacity_key.is_some()
            && receipt.proves(self.event_opacity_tokens.len())
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_opacity_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Opacity]);
        }
        evidence
    }
}

fn timeline_event_count(layout: &TimelineDiagramLayout) -> usize {
    layout
        .sections
        .iter()
        .flat_map(|section| section.tasks.iter())
        .chain(layout.orphan_tasks.iter())
        .map(|task| task.events.len())
        .sum()
}

fn typed_opacity_token(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<Box<str>> {
    let origin = style.opacity_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Opacity)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.opacity_resolution().specified() {
        Specified::Value(value) => Some(value.to_string().into_boxed_str()),
        Specified::Unspecified | Specified::Clear => None,
    }
}

/// Writer-owned proof that every Timeline event wrapper emitted its canonical opacity state.
#[derive(Debug)]
pub(crate) struct TimelineEventOpacityThemeReceipt {
    expected_event_count: usize,
    next_event_index: usize,
    attributes_match: bool,
}

impl TimelineEventOpacityThemeReceipt {
    fn new(event_count: usize) -> Self {
        Self {
            expected_event_count: event_count,
            next_event_index: 0,
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_event(
        &mut self,
        event_index: usize,
        emitted_opacity_token: Option<&str>,
        expected_opacity_token: Option<&str>,
    ) {
        if event_index != self.next_event_index || event_index >= self.expected_event_count {
            self.attributes_match = false;
            return;
        }
        self.next_event_index = self.next_event_index.saturating_add(1);
        self.attributes_match &= emitted_opacity_token == expected_opacity_token;
    }

    fn proves(&self, expected_event_count: usize) -> bool {
        self.expected_event_count == expected_event_count
            && self.next_event_index == expected_event_count
            && self.attributes_match
    }
}

#[derive(Debug, Default)]
struct TimelineEventRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    opacity_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::TimelineEventOpacityThemeReceipt;

    #[test]
    fn event_opacity_receipt_requires_each_terminal_event_once() {
        let mut complete = TimelineEventOpacityThemeReceipt::new(1);
        complete.record_checkpointed_event(0, Some("0.5"), Some("0.5"));
        assert!(complete.proves(1));

        let mut incomplete = TimelineEventOpacityThemeReceipt::new(2);
        incomplete.record_checkpointed_event(0, Some("0.5"), Some("0.5"));
        assert!(!incomplete.proves(2));

        let mut duplicate = TimelineEventOpacityThemeReceipt::new(1);
        duplicate.record_checkpointed_event(0, Some("0.5"), Some("0.5"));
        duplicate.record_checkpointed_event(0, Some("0.5"), Some("0.5"));
        assert!(!duplicate.proves(1));

        let mut out_of_order = TimelineEventOpacityThemeReceipt::new(2);
        out_of_order.record_checkpointed_event(1, Some("0.5"), Some("0.5"));
        out_of_order.record_checkpointed_event(0, Some("0.5"), Some("0.5"));
        assert!(!out_of_order.proves(2));

        let mut mismatch = TimelineEventOpacityThemeReceipt::new(1);
        mismatch.record_checkpointed_event(0, Some("1"), Some("0.9999995"));
        assert!(!mismatch.proves(1));
    }
}
