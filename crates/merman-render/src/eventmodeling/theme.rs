use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::EventModelingDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EventModelingTextOccurrences {
    swimlanes: usize,
    boxes: usize,
}

impl EventModelingTextOccurrences {
    fn from_layout(layout: &EventModelingDiagramLayout) -> Self {
        Self {
            swimlanes: layout
                .swimlanes
                .iter()
                .filter(|swimlane| !swimlane.label.trim().is_empty())
                .count(),
            boxes: layout
                .boxes
                .iter()
                .filter(|box_layout| !box_layout.text.trim().is_empty())
                .count(),
        }
    }

    const fn total(self) -> usize {
        self.swimlanes.saturating_add(self.boxes)
    }
}

#[derive(Debug, Default)]
struct EventModelingTextRuleObservation {
    applicable: bool,
    config_owned: bool,
    pending_fill: Option<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Final Event Modeling text paint shared by terminal emission and family evidence.
#[derive(Debug)]
pub(crate) struct EventModelingTextThemePlan {
    typed_fill: Option<DirectStaticPaint>,
    occurrences: EventModelingTextOccurrences,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
    terminal_receipt: OnceLock<EventModelingTextThemeReceipt>,
}

impl EventModelingTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &EventModelingDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let occurrences = EventModelingTextOccurrences::from_layout(layout);
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_occurrences(occurrences));
        };

        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.textColor",
        );
        let style = (occurrences.total() != 0)
            .then(|| {
                theme.text_style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )
            })
            .transpose()?;
        let winner_properties = style
            .as_ref()
            .into_iter()
            .flat_map(|style| style.winner_rule_properties())
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let typed_fill = (!config_owns_fill)
            .then(|| {
                style.as_ref().and_then(|style| {
                    resolve_direct_static_fill(
                        theme,
                        style,
                        &[ThemeTarget::Text],
                        DirectStaticSelectorDomain::Unqualified,
                    )
                })
            })
            .flatten();

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, EventModelingTextRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(occurrences.total()) {
                        continue;
                    }
                    if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        observation.applicable = true;
                        observation.config_owned = true;
                        continue;
                    }

                    if selector != (FamilyThemeSelectorShape::Static { variant: None }) {
                        observation.applicable = true;
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                observation
                                    .residual
                                    .get_or_insert(unsupported_residual_for_facet(facet));
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::LegacyCompatibility => {
                                observation.incomplete = true;
                            }
                        }
                        continue;
                    }

                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }
                    observation.applicable = true;

                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Fill(_))
                            if typed_fill
                                .as_ref()
                                .is_some_and(|fill| fill.rule_index() == rule_index) =>
                        {
                            observation.pending_fill =
                                typed_fill.as_ref().map(DirectStaticPaint::capability);
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
                    target: ThemeTarget::Text,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if occurrences.total() == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if occurrences.total() == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Title,
                    ..
                }
                | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                }
                | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Event Modeling has no visual title terminal. Accessibility metadata is root
                    // chrome and is intentionally not the semantic Title styling target.
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
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
                target: ThemeTarget::Text,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if let Some(capability) = observation.pending_fill {
                pending.insert(key, capability);
            } else if observation.config_owned {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            typed_fill,
            occurrences,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(layout: &EventModelingDiagramLayout) -> Self {
        Self::baseline_from_occurrences(EventModelingTextOccurrences::from_layout(layout))
    }

    fn baseline_from_occurrences(occurrences: EventModelingTextOccurrences) -> Self {
        Self {
            typed_fill: None,
            occurrences,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn terminal_fill<'a>(&'a self, configured_fill: &'a str) -> &'a str {
        self.typed_fill
            .as_ref()
            .map_or(configured_fill, DirectStaticPaint::css)
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        configured_fill: &str,
    ) -> EventModelingTextThemeReceipt {
        EventModelingTextThemeReceipt::new(self.occurrences, self.terminal_fill(configured_fill))
    }

    pub(crate) fn record_terminal(&self, receipt: EventModelingTextThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some() {
            for (key, capability) in &self.pending {
                evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
            }
        }
        evidence
    }
}

/// Writer-owned proof of both Event Modeling terminal text roles and their exact paint values.
#[derive(Debug)]
pub(crate) struct EventModelingTextThemeReceipt {
    expected: EventModelingTextOccurrences,
    emitted: EventModelingTextOccurrences,
    expected_fill: Box<str>,
    terminal_matches: bool,
}

impl EventModelingTextThemeReceipt {
    fn new(expected: EventModelingTextOccurrences, expected_fill: &str) -> Self {
        Self {
            expected,
            emitted: EventModelingTextOccurrences::default(),
            expected_fill: expected_fill.into(),
            terminal_matches: true,
        }
    }

    pub(crate) fn record_swimlane_text(&mut self, text: &str, emitted_fill: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.swimlanes = self.emitted.swimlanes.saturating_add(1);
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
    }

    pub(crate) fn record_box_text(&mut self, text: &str, emitted_color: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted.boxes = self.emitted.boxes.saturating_add(1);
        self.terminal_matches &= emitted_color == self.expected_fill.as_ref();
    }

    fn proves_complete(&self) -> bool {
        self.emitted == self.expected && self.terminal_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eventmodeling_text_receipt_requires_every_non_empty_terminal_role() {
        let expected = EventModelingTextOccurrences {
            swimlanes: 1,
            boxes: 1,
        };
        let mut missing_box = EventModelingTextThemeReceipt::new(expected, "#123456");
        missing_box.record_swimlane_text("UI/Automation", "#123456");
        assert!(!missing_box.proves_complete());

        let mut mismatched_box = EventModelingTextThemeReceipt::new(expected, "#123456");
        mismatched_box.record_swimlane_text("UI/Automation", "#123456");
        mismatched_box.record_box_text("View", "#abcdef");
        assert!(!mismatched_box.proves_complete());

        let mut extra_swimlane = EventModelingTextThemeReceipt::new(expected, "#123456");
        extra_swimlane.record_swimlane_text("UI/Automation", "#123456");
        extra_swimlane.record_swimlane_text("UI/A: Shop", "#123456");
        extra_swimlane.record_box_text("View", "#123456");
        assert!(!extra_swimlane.proves_complete());

        let mut complete = EventModelingTextThemeReceipt::new(expected, "#123456");
        complete.record_swimlane_text("", "wrong-but-empty");
        complete.record_box_text("   ", "wrong-but-empty");
        complete.record_swimlane_text("UI/Automation", "#123456");
        complete.record_box_text("View", "#123456");
        assert!(complete.proves_complete());
    }
}
