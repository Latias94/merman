use std::collections::{BTreeMap, BTreeSet};

use merman_core::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeOutcomes, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const ARCH_EDGE_COLOR_PATH: &str = "themeVariables.archEdgeColor";
const LINE_COLOR_PATH: &str = "themeVariables.lineColor";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchitectureEdgeStroke {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

#[derive(Debug, Default)]
struct ArchitectureEdgeRuleObservation {
    applicable: bool,
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Direct Architecture edge-path stroke owned by the family renderer.
#[derive(Debug)]
pub(super) struct ArchitectureEdgeStrokePlan {
    stroke: Option<ArchitectureEdgeStroke>,
    pending_key: Option<FamilyThemeMechanismKey>,
}

impl ArchitectureEdgeStrokePlan {
    pub(super) fn baseline() -> Self {
        Self {
            stroke: None,
            pending_key: None,
        }
    }

    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        effective_config: &PresentationConfig,
        edge_count: usize,
        work_meter: &OperationWorkMeter,
        evidence: &mut FamilyThemeOutcomes,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline();
        let config_owns_stroke = mermaid_owns_edge_stroke(effective_config);
        let has_ordinal_edge_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Edge
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && rule.ordinal().is_some()
        });
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut first_candidate = None::<Option<ArchitectureEdgeStroke>>;
        let mut candidates_match = true;

        if edge_count > 0 {
            if has_ordinal_edge_rules {
                for ordinal in 1..=edge_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Edge,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    let candidate = observe_style(theme, &style, &mut winner_properties);
                    match first_candidate.as_ref() {
                        None => first_candidate = Some(candidate),
                        Some(first) => candidates_match &= first == &candidate,
                    }
                }
            } else {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )?;
                first_candidate = Some(observe_style(theme, &style, &mut winner_properties));
            }
        }

        let stroke = candidates_match
            .then(|| first_candidate.flatten())
            .flatten();
        let stroke_rule = stroke.as_ref().map(|stroke| stroke.rule_index);
        let mut observations = BTreeMap::<usize, ArchitectureEdgeRuleObservation>::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if config_owns_stroke
                        && resolved_style_property_for_facet(facet) == ResolvedStyleProperty::Stroke
                    {
                        continue;
                    }
                    if !selector.ordinal_domain_intersects_occurrence_count(edge_count) {
                        continue;
                    }
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
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if stroke_rule == Some(rule_index) => {
                            observation.pending = true;
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Edge,
                } => {
                    // Reconciled below from the final style of each actual edge occurrence.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                } => {
                    // Reconciled below from the final style of each actual edge occurrence.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(edge_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Edge,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Selector-qualified, mixed-facet, or compatibility routes remain fail-closed.
            } else if observation.pending {
                debug_assert!(plan.pending_key.is_none());
                plan.pending_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        if !config_owns_stroke && plan.pending_key.is_some() {
            plan.stroke = stroke;
        }
        Ok(plan)
    }

    pub(super) fn terminal_stroke(&self) -> Option<&str> {
        self.stroke.as_ref().map(|stroke| stroke.css.as_ref())
    }

    pub(super) fn finish_evidence(&self, evidence: &mut FamilyThemeOutcomes, emitted: bool) {
        if emitted
            && let (Some(key), Some(stroke)) = (self.pending_key.clone(), self.stroke.as_ref())
        {
            evidence.mark_applied_with_capabilities(key, [stroke.capability]);
        }
    }
}

fn observe_style(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
) -> Option<ArchitectureEdgeStroke> {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    typed_static_stroke(theme, style)
}

fn typed_static_stroke(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<ArchitectureEdgeStroke> {
    let paint = resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Edge],
        DirectStaticSelectorDomain::Unqualified,
    )?;
    let (css, rule_index, capability) = paint.into_parts();
    Some(ArchitectureEdgeStroke {
        css,
        rule_index,
        capability,
    })
}

fn mermaid_owns_edge_stroke(config: &PresentationConfig) -> bool {
    if merman_core::__private::config_paint_overrides_typed_default(config, ARCH_EDGE_COLOR_PATH) {
        return true;
    }
    merman_core::__private::config_paint_overrides_typed_default(config, LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_COLOR_PATH) == config.get_str(LINE_COLOR_PATH)
}
