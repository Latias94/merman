use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_css_number_or_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackPlan, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
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
    pending_fill: Option<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Final Event Modeling text styling shared by terminal emission and family evidence.
#[derive(Debug)]
pub(crate) struct EventModelingTextThemePlan {
    typed_fill: Option<DirectStaticPaint>,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Option<Box<str>>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    occurrences: EventModelingTextOccurrences,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
    terminal_verified: OnceLock<()>,
}

impl EventModelingTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &EventModelingDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let occurrences = EventModelingTextOccurrences::from_layout(layout);
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let Some(theme) = theme else {
            let configured_font_size_css = config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .map(String::into_boxed_str);
            return Ok(Self::baseline_from_occurrences(
                occurrences,
                inherited_font_stack,
                configured_font_size_css,
                false,
                false,
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
        let mut typed_font_size = false;
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
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter => {}
                // Base typography is reconciled by property in `finish_evidence`. The shared
                // inherited-font plan records every Unsupported property so mixed requests do
                // not make a supported sibling disappear from the terminal writer.
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet { .. }
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
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        let config_owns_font_size = typed_font_size
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size && !config_owns_font_size;
        let font_size_css = if typed_font_size_active {
            Some(format!("{}px", theme.typography().font_size_px()).into_boxed_str())
        } else {
            config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .map(String::into_boxed_str)
        };

        Ok(Self {
            typed_fill,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: typed_font_size,
            typed_font_size_active,
            occurrences,
            evidence,
            pending,
            terminal_verified: OnceLock::new(),
        })
    }

    fn baseline_from_occurrences(
        occurrences: EventModelingTextOccurrences,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Option<Box<str>>,
        typed_font_size_requested: bool,
        typed_font_size_active: bool,
    ) -> Self {
        Self {
            typed_fill: None,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            occurrences,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_verified: OnceLock::new(),
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
        EventModelingTextThemeReceipt::new(
            self.occurrences,
            self.terminal_fill(configured_fill),
            self.inherited_font_stack.font_family_css(),
            self.font_size_css.as_deref(),
        )
    }

    pub(crate) fn record_terminal(&self, receipt: EventModelingTextThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_verified.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let terminal_verified = self.terminal_verified.get().is_some();
        if terminal_verified {
            for (key, capability) in &self.pending {
                evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
            }
        }
        let has_visible_terminals = self.occurrences.total() != 0;
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_terminals);
        let properties = [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
            ),
        ];
        for (property, requested, active) in properties {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if !has_visible_terminals || !active {
                evidence.mark_not_applicable(key);
            } else if terminal_verified {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
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
    stylesheet_emissions: usize,
    terminal_matches: bool,
}

impl EventModelingTextThemeReceipt {
    fn new(
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
            stylesheet_emissions: 0,
            terminal_matches: true,
        }
    }

    /// Write the exact scoped stylesheet from the values sealed into this writer-owned receipt.
    /// Evidence never reparses the resulting CSS.
    pub(crate) fn write_stylesheet(
        &mut self,
        out: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
    ) {
        self.stylesheet_emissions = self.stylesheet_emissions.saturating_add(1);
        let _ = write!(
            out,
            "#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {};",
            self.expected_font_family
        );
        if let Some(font_size) = self.expected_font_size.as_deref() {
            let _ = write!(out, " font-size: {font_size};");
        }
        let _ = write!(
            out,
            " color: {}; }}#{diagram_id} .em-relation {{ fill: none; }}",
            self.expected_fill
        );
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
        self.stylesheet_emissions == 1 && self.emitted == self.expected && self.terminal_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_receipt_stylesheet(receipt: &mut EventModelingTextThemeReceipt) -> String {
        let mut stylesheet = String::new();
        receipt.write_stylesheet(&mut stylesheet, "event-receipt");
        stylesheet
    }

    #[test]
    fn eventmodeling_text_receipt_requires_every_non_empty_terminal_role() {
        let expected = EventModelingTextOccurrences {
            swimlanes: 1,
            boxes: 1,
        };
        let mut missing_box = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
        let _ = write_receipt_stylesheet(&mut missing_box);
        missing_box.record_swimlane_text("UI/Automation", "#123456");
        assert!(!missing_box.proves_complete());

        let mut mismatched_box = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
        let _ = write_receipt_stylesheet(&mut mismatched_box);
        mismatched_box.record_swimlane_text("UI/Automation", "#123456");
        mismatched_box.record_box_text("View", "#abcdef");
        assert!(!mismatched_box.proves_complete());

        let mut extra_swimlane = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
        let _ = write_receipt_stylesheet(&mut extra_swimlane);
        extra_swimlane.record_swimlane_text("UI/Automation", "#123456");
        extra_swimlane.record_swimlane_text("UI/A: Shop", "#123456");
        extra_swimlane.record_box_text("View", "#123456");
        assert!(!extra_swimlane.proves_complete());

        let mut complete = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
        let _ = write_receipt_stylesheet(&mut complete);
        complete.record_swimlane_text("", "wrong-but-empty");
        complete.record_box_text("   ", "wrong-but-empty");
        complete.record_swimlane_text("UI/Automation", "#123456");
        complete.record_box_text("View", "#123456");
        assert!(complete.proves_complete());
    }

    #[test]
    fn eventmodeling_text_receipt_requires_one_exact_typography_emission() {
        let expected = EventModelingTextOccurrences {
            swimlanes: 1,
            boxes: 1,
        };
        let mut complete =
            EventModelingTextThemeReceipt::new(expected, "#123456", "Fira Sans", Some("24px"));
        let stylesheet = write_receipt_stylesheet(&mut complete);
        assert!(stylesheet.contains(
            "#event-receipt .em-swimlane text,#event-receipt .em-box span { font-family: Fira Sans; font-size: 24px; color: #123456; }"
        ));
        assert!(stylesheet.contains("#event-receipt .em-relation { fill: none; }"));
        complete.record_swimlane_text("UI/Automation", "#123456");
        complete.record_box_text("View", "#123456");
        assert!(complete.proves_complete());

        let mut missing =
            EventModelingTextThemeReceipt::new(expected, "#123456", "Fira Sans", Some("24px"));
        missing.record_swimlane_text("UI/Automation", "#123456");
        missing.record_box_text("View", "#123456");
        assert!(!missing.proves_complete());

        let mut duplicate =
            EventModelingTextThemeReceipt::new(expected, "#123456", "Fira Sans", Some("24px"));
        let _ = write_receipt_stylesheet(&mut duplicate);
        let _ = write_receipt_stylesheet(&mut duplicate);
        assert!(!duplicate.proves_complete());
    }
}
