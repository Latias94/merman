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
    css_binding: super::EventModelingCssBinding,
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
    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        mut css_binding: super::EventModelingCssBinding,
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
                css_binding,
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

        if let Some(fill) = typed_fill {
            css_binding.text_color = fill.css().to_owned();
        }
        Ok(Self {
            css_binding,
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
        css_binding: super::EventModelingCssBinding,
        occurrences: EventModelingTextOccurrences,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Option<Box<str>>,
        typed_font_size_requested: bool,
        typed_font_size_active: bool,
    ) -> Self {
        Self {
            css_binding,
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

    pub(crate) fn css_binding(&self) -> &super::EventModelingCssBinding {
        &self.css_binding
    }

    pub(crate) fn begin_terminal_receipt(&self) -> EventModelingTextThemeReceipt {
        EventModelingTextThemeReceipt::new(
            self.occurrences,
            &self.css_binding.text_color,
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
    output_succeeded: bool,
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
            output_succeeded: true,
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
        self.output_succeeded &= write!(
            out,
            "#{diagram_id} {{ fill: {}; color: {}; }}#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {};",
            self.expected_fill, self.expected_fill, self.expected_font_family
        )
        .is_ok();
        if let Some(font_size) = self.expected_font_size.as_deref() {
            self.output_succeeded &= write!(out, " font-size: {font_size};").is_ok();
        }
        self.output_succeeded &=
            write!(out, " }}#{diagram_id} .em-relation {{ fill: none; }}").is_ok();
    }

    /// The text inherits paint from the SVG root and typography from its scoped role selector.
    pub(crate) fn write_swimlane_text_start(
        &mut self,
        out: &mut impl std::fmt::Write,
        text: &str,
        x: impl std::fmt::Display,
        y: impl std::fmt::Display,
    ) {
        self.output_succeeded &=
            write!(out, r#"<text font-weight="bold" x="{x}" y="{y}">"#).is_ok();
        if !text.trim().is_empty() {
            self.emitted.swimlanes = self.emitted.swimlanes.saturating_add(1);
        }
    }

    /// XHTML wrappers own layout only, leaving color available to ordinary author CSS.
    pub(crate) fn write_box_text_start(&mut self, out: &mut impl std::fmt::Write, text: &str) {
        self.output_succeeded &= out
            .write_str(r#"<div xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><span style="display: table-cell; text-align: center; vertical-align: middle;">"#)
            .is_ok();
        if !text.trim().is_empty() {
            self.emitted.boxes = self.emitted.boxes.saturating_add(1);
        }
    }

    fn proves_complete(&self) -> bool {
        self.stylesheet_emissions == 1 && self.emitted == self.expected && self.output_succeeded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_text_receipt_consumes_prepared_raw_paint_and_font_tokens() {
        let config = MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {
                "textColor": "var(--text)", "fontFamily": "Fira Sans", "fontSize": "1.5em"
            }
        }));
        let binding = super::super::EventModelingCssBinding::resolve(config.as_value());
        let layout = super::super::layout_eventmodeling_diagram_typed_with_binding(
            &merman_core::diagrams::eventmodeling::EventModelingDiagramRenderModel::default(),
            &binding,
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .expect("empty Event Modeling layout");
        let work = OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
        let plan = EventModelingTextThemePlan::resolve_with_binding(
            None, &config, binding, &layout, &work,
        )
        .expect("prepared baseline text");
        let mut receipt = plan.begin_terminal_receipt();
        let css = write_receipt_stylesheet(&mut receipt);
        assert!(css.contains("fill: var(--text); color: var(--text);"));
        assert!(css.contains("font-family: Fira Sans; font-size: 1.5em;"));
        assert!(plan.record_terminal(receipt));
    }

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
        let mut output = String::new();
        missing_box.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
        assert!(!missing_box.proves_complete());

        for (swimlanes, boxes) in [(0, 1), (2, 1), (1, 2)] {
            let mut incomplete = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
            let _ = write_receipt_stylesheet(&mut incomplete);
            let mut output = String::new();
            for _ in 0..swimlanes {
                incomplete.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
            }
            for _ in 0..boxes {
                incomplete.write_box_text_start(&mut output, "View");
            }
            assert!(!incomplete.proves_complete());
        }

        let mut complete = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
        let _ = write_receipt_stylesheet(&mut complete);
        let mut output = String::new();
        complete.write_swimlane_text_start(&mut output, "", 0, 0);
        complete.write_box_text_start(&mut output, "   ");
        complete.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
        complete.write_box_text_start(&mut output, "View");
        assert!(complete.proves_complete());
    }

    #[test]
    fn eventmodeling_text_receipt_writes_inheriting_text_openings() {
        let mut receipt = EventModelingTextThemeReceipt::new(
            EventModelingTextOccurrences {
                swimlanes: 1,
                boxes: 1,
            },
            "#123456",
            "Fira Sans",
            Some("24px"),
        );
        let _ = write_receipt_stylesheet(&mut receipt);
        let mut swimlane = String::new();
        receipt.write_swimlane_text_start(&mut swimlane, "UI/Automation", 30, 45);
        assert_eq!(swimlane, r#"<text font-weight="bold" x="30" y="45">"#);
        let mut box_text = String::new();
        receipt.write_box_text_start(&mut box_text, "View");
        assert_eq!(
            box_text,
            r#"<div xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><span style="display: table-cell; text-align: center; vertical-align: middle;">"#
        );
        assert!(receipt.proves_complete());
    }

    #[test]
    fn eventmodeling_text_receipt_rejects_failed_output() {
        struct FailedOutput;
        impl std::fmt::Write for FailedOutput {
            fn write_str(&mut self, _: &str) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        let expected = EventModelingTextOccurrences {
            swimlanes: 1,
            boxes: 1,
        };
        for failing_role in 0..3 {
            let mut receipt = EventModelingTextThemeReceipt::new(expected, "#123456", "", None);
            let mut output = String::new();
            if failing_role == 0 {
                receipt.write_stylesheet(&mut FailedOutput, "event-receipt");
            } else {
                receipt.write_stylesheet(&mut output, "event-receipt");
            }
            if failing_role == 1 {
                receipt.write_swimlane_text_start(&mut FailedOutput, "UI/Automation", 30, 30);
            } else {
                receipt.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
            }
            if failing_role == 2 {
                receipt.write_box_text_start(&mut FailedOutput, "View");
            } else {
                receipt.write_box_text_start(&mut output, "View");
            }
            assert!(!receipt.proves_complete());
        }
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
            "#event-receipt .em-swimlane text,#event-receipt .em-box span { font-family: Fira Sans; font-size: 24px; }"
        ));
        assert!(stylesheet.contains("#event-receipt { fill: #123456; color: #123456; }"));
        assert!(stylesheet.contains("#event-receipt .em-relation { fill: none; }"));
        let mut output = String::new();
        complete.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
        complete.write_box_text_start(&mut output, "View");
        assert!(complete.proves_complete());

        let mut missing =
            EventModelingTextThemeReceipt::new(expected, "#123456", "Fira Sans", Some("24px"));
        missing.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
        missing.write_box_text_start(&mut output, "View");
        assert!(!missing.proves_complete());

        let mut duplicate =
            EventModelingTextThemeReceipt::new(expected, "#123456", "Fira Sans", Some("24px"));
        let _ = write_receipt_stylesheet(&mut duplicate);
        let _ = write_receipt_stylesheet(&mut duplicate);
        duplicate.write_swimlane_text_start(&mut output, "UI/Automation", 30, 30);
        duplicate.write_box_text_start(&mut output, "View");
        assert!(!duplicate.proves_complete());
    }
}
