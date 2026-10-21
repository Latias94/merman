use std::collections::{BTreeMap, BTreeSet};

use merman_core::MermaidConfig;

use crate::config::config_css_number_or_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackPlan, TerminalVariantDomain,
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
    evidence: FamilyThemeOutcomes,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
}

impl EventModelingTextThemePlan {
    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        mut css_binding: super::EventModelingCssBinding,
        layout: &EventModelingDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let occurrences = theme
            .map(|_| EventModelingTextOccurrences::from_layout(layout))
            .unwrap_or_default();
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

        let mut evidence = FamilyThemeOutcomes::default();
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
                    if !selector.ordinal_domain_intersects_occurrence_count(occurrences.total())
                        || matches!(selector,
                            FamilyThemeSelectorShape::Static { variant: Some(variant) }
                            | FamilyThemeSelectorShape::Ordinal { variant: Some(variant), .. }
                            if variant != ThemeVariant::Default)
                    {
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
                            FamilyThemeDisposition::TypedAdapter => {
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
                        (FamilyThemeDisposition::TypedAdapter, _) => {
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
                // Base typography is reconciled by property in `record_evidence`. The shared
                // inherited-font plan records every Unsupported property so mixed requests do
                // not make a supported sibling disappear from the terminal writer.
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let source_owned_fill = config_owns_fill.then(|| vec![true; occurrences.total()]);
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Text,
                TerminalVariantDomain::uniform(occurrences.total(), ThemeVariant::Default),
            )
            .with_source_owned_fill(source_owned_fill.as_deref().unwrap_or_default())],
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
            evidence: FamilyThemeOutcomes::default(),
            pending: BTreeMap::new(),
        }
    }

    pub(crate) fn css_binding(&self) -> &super::EventModelingCssBinding {
        &self.css_binding
    }

    pub(crate) fn write_stylesheet(
        &self,
        out: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
    ) {
        let fill = &self.css_binding.text_color;
        let font = self.inherited_font_stack.font_family_css();
        let _ = write!(
            out,
            "#{diagram_id} {{ fill: {fill}; color: {fill}; }}#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {font};"
        );
        if let Some(font_size) = self.font_size_css.as_deref() {
            let _ = write!(out, " font-size: {font_size};");
        }
        let _ = write!(out, " }}#{diagram_id} .em-relation {{ fill: none; }}");
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for (key, capability) in &self.pending {
            evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
        }
        let has_visible_terminals = self.occurrences.total() != 0;
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, has_visible_terminals);
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
            } else {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
        }
    }
}
