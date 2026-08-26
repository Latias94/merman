use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_css_number_or_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
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
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Option<Box<str>>,
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
        let inherited_font_stack = InheritedFontStackPlan::resolve(theme, effective_config);
        let font_size_css = config_css_number_or_string(
            effective_config.as_value(),
            &["themeVariables", "fontSize"],
        )
        .map(String::into_boxed_str);
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_occurrences(
                occurrences,
                inherited_font_stack,
                font_size_css,
            ));
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
                    // Reconciled below from the final text style of each actual terminal.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    // Reconciled below from the final text style of each actual terminal.
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

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Text,
                TerminalVariantDomain::uniform(occurrences.total(), ThemeVariant::Default),
            )],
            work_meter,
        )?;

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
            inherited_font_stack,
            font_size_css,
            occurrences,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    fn baseline_from_occurrences(
        occurrences: EventModelingTextOccurrences,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Option<Box<str>>,
    ) -> Self {
        Self {
            typed_fill: None,
            inherited_font_stack,
            font_size_css,
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

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        configured_fill: &str,
    ) -> EventModelingTextThemeReceipt {
        EventModelingTextThemeReceipt::with_font_family(
            self.occurrences,
            self.terminal_fill(configured_fill),
            self.font_family_css(),
            self.font_size_css.as_deref(),
        )
    }

    pub(crate) fn record_terminal(&self, receipt: EventModelingTextThemeReceipt) -> bool {
        receipt.proves_complete()
            && receipt.proves_font_stack(self.inherited_font_stack.outcome())
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some() {
            for (key, capability) in &self.pending {
                evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
            }
        }
        if let Some(receipt) = self.terminal_receipt.get() {
            let typography_key = FamilyThemeMechanismKey::Typography;
            if self.occurrences.total() == 0 {
                evidence.mark_not_applicable(typography_key);
            } else {
                match self.inherited_font_stack.outcome() {
                    InheritedFontStackOutcome::Typed
                        if receipt.proves_font_stack(InheritedFontStackOutcome::Typed) =>
                    {
                        evidence.mark_applied_with_capabilities(
                            typography_key,
                            [ThemeCapability::Typography],
                        )
                    }
                    InheritedFontStackOutcome::ConfigOwned
                        if receipt.proves_font_stack(InheritedFontStackOutcome::ConfigOwned) =>
                    {
                        evidence.mark_not_applicable(typography_key)
                    }
                    InheritedFontStackOutcome::Typed
                    | InheritedFontStackOutcome::ConfigOwned
                    | InheritedFontStackOutcome::Unsupported => evidence.mark_residual(
                        typography_key,
                        FamilyThemeResidualReason::UnsupportedTypography,
                    ),
                    InheritedFontStackOutcome::Inactive => {}
                }
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
    expected_font_family: Box<str>,
    expected_font_size: Option<Box<str>>,
    emitted_font_family: Option<Box<str>>,
    emitted_font_size: Option<Box<str>>,
    terminal_matches: bool,
}

impl EventModelingTextThemeReceipt {
    fn with_font_family(
        expected: EventModelingTextOccurrences,
        expected_fill: &str,
        expected_font_family: &str,
        expected_font_size: Option<&str>,
    ) -> Self {
        Self {
            expected,
            emitted: EventModelingTextOccurrences::default(),
            expected_fill: expected_fill.into(),
            expected_font_family: expected_font_family.into(),
            expected_font_size: expected_font_size.map(Into::into),
            emitted_font_family: None,
            emitted_font_size: None,
            terminal_matches: true,
        }
    }

    /// Build the exact stylesheet owned by this receipt and record the single font-stack writer
    /// event at the same boundary. Evidence never reparses the resulting CSS.
    pub(crate) fn stylesheet(&mut self) -> String {
        if self.emitted_font_family.is_some() {
            self.terminal_matches = false;
        }
        self.emitted_font_family = Some(self.expected_font_family.clone());
        if self.emitted_font_size.is_some() {
            self.terminal_matches = false;
        }
        self.emitted_font_size = self.expected_font_size.clone();
        let font_size = self
            .emitted_font_size
            .as_deref()
            .map(|value| format!(" font-size: {value};"))
            .unwrap_or_default();
        format!(
            ".em-swimlane text,.em-box span {{ font-family: {};{} color: {}; }}\
.em-relation {{ fill: none; }}",
            self.expected_font_family, font_size, self.expected_fill
        )
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

    fn proves_font_stack(&self, outcome: InheritedFontStackOutcome) -> bool {
        if outcome == InheritedFontStackOutcome::Inactive {
            return true;
        }
        self.emitted_font_family.as_deref() == Some(self.expected_font_family.as_ref())
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
        let mut missing_box =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "", None);
        missing_box.record_swimlane_text("UI/Automation", "#123456");
        assert!(!missing_box.proves_complete());

        let mut mismatched_box =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "", None);
        mismatched_box.record_swimlane_text("UI/Automation", "#123456");
        mismatched_box.record_box_text("View", "#abcdef");
        assert!(!mismatched_box.proves_complete());

        let mut extra_swimlane =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "", None);
        extra_swimlane.record_swimlane_text("UI/Automation", "#123456");
        extra_swimlane.record_swimlane_text("UI/A: Shop", "#123456");
        extra_swimlane.record_box_text("View", "#123456");
        assert!(!extra_swimlane.proves_complete());

        let mut complete =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "", None);
        complete.record_swimlane_text("", "wrong-but-empty");
        complete.record_box_text("   ", "wrong-but-empty");
        complete.record_swimlane_text("UI/Automation", "#123456");
        complete.record_box_text("View", "#123456");
        assert!(complete.proves_complete());
    }

    #[test]
    fn eventmodeling_text_receipt_requires_one_exact_font_stack_emission() {
        let expected = EventModelingTextOccurrences {
            swimlanes: 1,
            boxes: 1,
        };
        let mut complete =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "Fira Sans", None);
        let stylesheet = complete.stylesheet();
        assert!(stylesheet.contains(
            ".em-swimlane text,.em-box span { font-family: Fira Sans; color: #123456; }"
        ));
        complete.record_swimlane_text("UI/Automation", "#123456");
        complete.record_box_text("View", "#123456");
        assert!(complete.proves_complete());
        assert!(complete.proves_font_stack(InheritedFontStackOutcome::Typed));

        let mut missing =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "Fira Sans", None);
        missing.record_swimlane_text("UI/Automation", "#123456");
        missing.record_box_text("View", "#123456");
        assert!(!missing.proves_font_stack(InheritedFontStackOutcome::Typed));

        let mut duplicate =
            EventModelingTextThemeReceipt::with_font_family(expected, "#123456", "Fira Sans", None);
        let _ = duplicate.stylesheet();
        let _ = duplicate.stylesheet();
        assert!(!duplicate.proves_complete());
    }
}
