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

#[derive(Debug, Clone)]
struct TimelineEventRadius {
    token: Box<str>,
    value_px: f64,
}

impl TimelineEventRadius {
    fn value(value: f32, node: &crate::model::TimelineNodeLayout) -> Self {
        let max_radius = node.width.max(1.0).min(node.height.max(1.0)) / 2.0;
        let normalized_value = if value == 0.0 { 0.0 } else { value };
        let authored_value_px = f64::from(normalized_value);
        let effective_value_px = authored_value_px.min(max_radius);
        let token = if authored_value_px > max_radius {
            effective_value_px.to_string()
        } else {
            normalized_value.to_string()
        };
        let value_px = token
            .parse::<f64>()
            .expect("Timeline event radius token must be a canonical finite number");
        Self {
            token: token.into_boxed_str(),
            value_px,
        }
    }
}

#[derive(Debug, Clone)]
struct TimelineEventTerminalExpectation {
    radius: Option<TimelineEventRadius>,
    radius_rule_index: Option<usize>,
    opacity_token: Option<Box<str>>,
    opacity_rule_index: Option<usize>,
}

impl TimelineEventTerminalExpectation {
    fn baseline() -> Self {
        Self {
            radius: None,
            radius_rule_index: None,
            opacity_token: None,
            opacity_rule_index: None,
        }
    }
}

/// Timeline event radius/opacity and evidence resolved in the exact terminal draw order.
#[derive(Debug)]
pub(crate) struct TimelineEventTheme {
    events: Box<[TimelineEventTerminalExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, TimelineEventPendingEvidence>,
    terminal_receipt: OnceLock<TimelineEventThemeReceipt>,
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
        let event_nodes = timeline_event_nodes(layout);
        let event_count = event_nodes.len();

        let mut events = vec![TimelineEventTerminalExpectation::baseline(); event_count];
        let mut winner_counts =
            BTreeMap::<(usize, crate::diagram_theme::ResolvedStyleProperty), usize>::new();
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
            collect_winners(&style, event_count, &mut winner_counts);
            for (event, node) in events.iter_mut().zip(event_nodes.iter().copied()) {
                apply_event_style(theme, &style, event, node);
            }
        } else {
            for (event_index, event) in events.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::TimelineEvent,
                    ThemeVariant::Default,
                    Some(event_index + 1),
                    work_meter,
                )?;
                collect_winners(&style, 1, &mut winner_counts);
                apply_event_style(theme, &style, event, event_nodes[event_index]);
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
                    if !selector.ordinal_domain_intersects_occurrence_count(event_count) {
                        continue;
                    }
                    let route_won = winner_counts
                        .contains_key(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if !route_won && !qualified_variant {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Radius) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(event_count)
                                && events
                                    .iter()
                                    .all(|event| event.radius_rule_index == Some(rule_index))
                            {
                                observation.radius_pending = true;
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Opacity) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(event_count)
                                && events
                                    .iter()
                                    .all(|event| event.opacity_rule_index == Some(rule_index))
                            {
                                observation.opacity_pending = true;
                            } else {
                                observation.incomplete = true;
                            }
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

        let mut pending = BTreeMap::new();
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
            } else if observation.radius_pending || observation.opacity_pending {
                let mut capabilities = BTreeSet::new();
                if observation.radius_pending {
                    capabilities.insert(ThemeCapability::RoundedGeometry);
                }
                if observation.opacity_pending {
                    capabilities.insert(ThemeCapability::Opacity);
                }
                pending.insert(
                    key,
                    TimelineEventPendingEvidence {
                        radius: observation.radius_pending,
                        opacity: observation.opacity_pending,
                        capabilities,
                    },
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            events: events.into_boxed_slice(),
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            events: Box::new([]),
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn opacity_token_for_event(&self, event_index: usize) -> Option<&str> {
        self.events
            .get(event_index)
            .and_then(|event| event.opacity_token.as_deref())
    }

    pub(crate) fn radius_for_event(&self, event_index: usize) -> Option<(&str, f64)> {
        self.events
            .get(event_index)
            .and_then(|event| event.radius.as_ref())
            .map(|radius| (radius.token.as_ref(), radius.value_px))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        is_redux_theme: bool,
    ) -> Option<TimelineEventThemeReceipt> {
        (!self.pending.is_empty()).then(|| {
            TimelineEventThemeReceipt::from_expectations_with_baseline(
                self.events.clone(),
                (!is_redux_theme).then_some(super::MERMAID_EVENT_RADIUS_TOKEN),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: TimelineEventThemeReceipt) -> bool {
        receipt.proves(self.events.len()) && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, pending) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index, pending) {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    pending.capabilities.iter().copied(),
                );
            }
        }
        evidence
    }
}

fn timeline_event_nodes(layout: &TimelineDiagramLayout) -> Vec<&crate::model::TimelineNodeLayout> {
    layout
        .sections
        .iter()
        .flat_map(|section| section.tasks.iter())
        .chain(layout.orphan_tasks.iter())
        .flat_map(|task| task.events.iter())
        .collect()
}

fn collect_winners(
    style: &crate::diagram_theme::ResolvedThemeStyle,
    occurrence_count: usize,
    winner_counts: &mut BTreeMap<(usize, crate::diagram_theme::ResolvedStyleProperty), usize>,
) {
    for (property, origin) in style.winner_rule_properties() {
        let key = (origin.rule_index(), property);
        *winner_counts.entry(key).or_default() += occurrence_count;
    }
}

fn apply_event_style(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    event: &mut TimelineEventTerminalExpectation,
    node: &crate::model::TimelineNodeLayout,
) {
    if let Some(origin) = style.radius_resolution().winner() {
        if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
            == Some(FamilyThemeDisposition::TypedAdapter)
        {
            match style.radius_resolution().specified() {
                Specified::Value(value) => {
                    event.radius = Some(TimelineEventRadius::value(*value, node));
                    event.radius_rule_index = Some(origin.rule_index());
                }
                Specified::Clear => {
                    event.radius = None;
                    event.radius_rule_index = Some(origin.rule_index());
                }
                Specified::Unspecified => {}
            }
        }
    }
    if let Some(origin) = style.opacity_resolution().winner() {
        if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Opacity)
            == Some(FamilyThemeDisposition::TypedAdapter)
        {
            event.opacity_token = match style.opacity_resolution().specified() {
                Specified::Value(value) => Some(value.to_string().into_boxed_str()),
                Specified::Unspecified | Specified::Clear => None,
            };
            event.opacity_rule_index = Some(origin.rule_index());
        }
    }
}

#[derive(Debug, Default)]
struct TimelineEventRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
    opacity_pending: bool,
}

#[derive(Debug, Default)]
struct TimelineEventPendingEvidence {
    radius: bool,
    opacity: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that every Timeline event emitted its canonical terminal state.
#[derive(Debug)]
pub(crate) struct TimelineEventThemeReceipt {
    expectations: Box<[TimelineEventTerminalExpectation]>,
    baseline_radius_token: Option<&'static str>,
    next_event_index: usize,
    attributes_match: bool,
    radius_rules: BTreeSet<usize>,
    opacity_rules: BTreeSet<usize>,
}

impl TimelineEventThemeReceipt {
    #[cfg(test)]
    fn new(event_count: usize) -> Self {
        Self::from_expectations(
            vec![TimelineEventTerminalExpectation::baseline(); event_count].into_boxed_slice(),
        )
    }

    #[cfg(test)]
    fn from_expectations(
        expectations: Box<[TimelineEventTerminalExpectation]>,
    ) -> TimelineEventThemeReceipt {
        Self::from_expectations_with_baseline(expectations, None)
    }

    fn from_expectations_with_baseline(
        expectations: Box<[TimelineEventTerminalExpectation]>,
        baseline_radius_token: Option<&'static str>,
    ) -> TimelineEventThemeReceipt {
        Self {
            expectations,
            baseline_radius_token,
            next_event_index: 0,
            attributes_match: true,
            radius_rules: BTreeSet::new(),
            opacity_rules: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_event(
        &mut self,
        event_index: usize,
        emitted_opacity_token: Option<&str>,
        emitted_opacity_matches: bool,
        emitted_radius_token: Option<&str>,
        emitted_radius_geometry_matches: bool,
    ) {
        if event_index != self.next_event_index || event_index >= self.expectations.len() {
            self.attributes_match = false;
            return;
        }

        let (opacity_matches, radius_matches, opacity_rule_index, radius_rule_index) = {
            let expected = &self.expectations[event_index];
            let expected_opacity_token = expected.opacity_token.as_deref();
            let expected_radius_token = expected
                .radius
                .as_ref()
                .map(|radius| radius.token.as_ref())
                .or(self.baseline_radius_token);
            (
                emitted_opacity_matches && emitted_opacity_token == expected_opacity_token,
                emitted_radius_geometry_matches
                    && expected_radius_token.map_or(emitted_radius_token.is_none(), |token| {
                        emitted_radius_token == Some(token)
                    }),
                expected.opacity_rule_index,
                expected.radius_rule_index,
            )
        };
        self.next_event_index += 1;
        self.attributes_match &= opacity_matches && radius_matches;
        if opacity_matches {
            if let Some(rule_index) = opacity_rule_index {
                self.opacity_rules.insert(rule_index);
            }
        }
        if radius_matches {
            if let Some(rule_index) = radius_rule_index {
                self.radius_rules.insert(rule_index);
            }
        }
    }

    fn proves(&self, expected_event_count: usize) -> bool {
        self.expectations.len() == expected_event_count
            && self.next_event_index == expected_event_count
            && self.attributes_match
    }

    fn proves_rule(&self, rule_index: usize, pending: &TimelineEventPendingEvidence) -> bool {
        (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.opacity || self.opacity_rules.contains(&rule_index))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TimelineEventPendingEvidence, TimelineEventRadius, TimelineEventTerminalExpectation,
        TimelineEventThemeReceipt,
    };

    #[test]
    fn timeline_event_receipt_requires_each_terminal_event_once() {
        let mut complete = TimelineEventThemeReceipt::new(1);
        complete.record_checkpointed_event(0, None, true, None, true);
        assert!(complete.proves(1));

        let mut incomplete = TimelineEventThemeReceipt::new(2);
        incomplete.record_checkpointed_event(0, None, true, None, true);
        assert!(!incomplete.proves(2));

        let mut duplicate = TimelineEventThemeReceipt::new(1);
        duplicate.record_checkpointed_event(0, None, true, None, true);
        duplicate.record_checkpointed_event(0, None, true, None, true);
        assert!(!duplicate.proves(1));

        let mut out_of_order = TimelineEventThemeReceipt::new(2);
        out_of_order.record_checkpointed_event(1, None, true, None, true);
        out_of_order.record_checkpointed_event(0, None, true, None, true);
        assert!(!out_of_order.proves(2));

        let mut mismatch = TimelineEventThemeReceipt::new(1);
        mismatch.record_checkpointed_event(0, Some("1"), true, None, true);
        assert!(!mismatch.proves(1));
    }

    #[test]
    fn timeline_event_receipt_tracks_matching_radius_rule() {
        let expectations = vec![TimelineEventTerminalExpectation {
            radius: Some(TimelineEventRadius {
                token: "12".into(),
                value_px: 12.0,
            }),
            radius_rule_index: Some(7),
            opacity_token: None,
            opacity_rule_index: None,
        }]
        .into_boxed_slice();
        let mut receipt = TimelineEventThemeReceipt::from_expectations(expectations);
        receipt.record_checkpointed_event(0, None, true, Some("12"), true);

        assert!(receipt.proves(1));
        assert!(receipt.radius_rules.contains(&7));
    }

    #[test]
    fn timeline_event_receipt_requires_matching_radius_and_opacity_tokens() {
        let expectations = vec![TimelineEventTerminalExpectation {
            radius: Some(TimelineEventRadius {
                token: "12".into(),
                value_px: 12.0,
            }),
            radius_rule_index: Some(7),
            opacity_token: Some("0.5".into()),
            opacity_rule_index: Some(7),
        }]
        .into_boxed_slice();
        let mut receipt = TimelineEventThemeReceipt::from_expectations(expectations.clone());
        receipt.record_checkpointed_event(0, Some("0.5"), true, Some("12"), true);

        assert!(receipt.proves(1));
        assert!(receipt.proves_rule(
            7,
            &TimelineEventPendingEvidence {
                radius: true,
                opacity: true,
                capabilities: std::collections::BTreeSet::new(),
            }
        ));

        let mut mismatch = TimelineEventThemeReceipt::from_expectations(expectations);
        mismatch.record_checkpointed_event(0, Some("0.5"), true, Some("13"), true);
        assert!(!mismatch.proves(1));

        let expectations = vec![TimelineEventTerminalExpectation {
            radius: None,
            radius_rule_index: None,
            opacity_token: Some("0.5".into()),
            opacity_rule_index: Some(7),
        }]
        .into_boxed_slice();
        let mut unobserved_opacity = TimelineEventThemeReceipt::from_expectations(expectations);
        unobserved_opacity.record_checkpointed_event(0, Some("0.5"), false, None, true);
        assert!(!unobserved_opacity.proves(1));
    }

    #[test]
    fn timeline_event_receipt_distinguishes_classic_and_redux_baselines() {
        let expectations = vec![TimelineEventTerminalExpectation::baseline()].into_boxed_slice();
        let mut classic = TimelineEventThemeReceipt::from_expectations_with_baseline(
            expectations.clone(),
            Some(crate::timeline::MERMAID_EVENT_RADIUS_TOKEN),
        );
        classic.record_checkpointed_event(0, None, true, None, true);
        assert!(!classic.proves(1));

        let mut redux =
            TimelineEventThemeReceipt::from_expectations_with_baseline(expectations, None);
        redux.record_checkpointed_event(0, None, true, None, true);
        assert!(redux.proves(1));

        let mut malformed = TimelineEventThemeReceipt::new(1);
        malformed.record_checkpointed_event(0, None, true, None, false);
        assert!(!malformed.proves(1));
    }
}
