use std::collections::BTreeMap;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect, Specified, ThemeTarget,
    ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{FamilyThemeEvidence, FamilyThemeResidualReason};

/// Family-owned variants for the real terminal occurrences of one semantic surface.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TerminalVariantDomain<'a> {
    Uniform {
        count: usize,
        variant: ThemeVariant,
    },
    GroupedAlternating {
        group_lengths: &'a [usize],
        odd: ThemeVariant,
        even: ThemeVariant,
    },
}

impl<'a> TerminalVariantDomain<'a> {
    pub(crate) const fn uniform(count: usize, variant: ThemeVariant) -> Self {
        Self::Uniform { count, variant }
    }

    pub(crate) const fn grouped_alternating(
        group_lengths: &'a [usize],
        odd: ThemeVariant,
        even: ThemeVariant,
    ) -> Self {
        Self::GroupedAlternating {
            group_lengths,
            odd,
            even,
        }
    }

    fn for_each(
        self,
        mut visit: impl FnMut(usize, ThemeVariant) -> Result<(), OperationWorkError>,
    ) -> Result<(), OperationWorkError> {
        match self {
            Self::Uniform { count, variant } => {
                for ordinal in 1..=count {
                    visit(ordinal, variant)?;
                }
            }
            Self::GroupedAlternating {
                group_lengths,
                odd,
                even,
            } => {
                let mut ordinal = 0usize;
                for group_length in group_lengths.iter().copied() {
                    for local_index in 0..group_length {
                        ordinal = ordinal.saturating_add(1);
                        visit(ordinal, if local_index % 2 == 0 { odd } else { even })?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum TerminalStyleResolution<'a> {
    Direct(ThemeTarget),
    Textual {
        terminal_target: ThemeTarget,
        owned_rule_targets: &'a [ThemeTarget],
    },
}

impl TerminalStyleResolution<'_> {
    const fn terminal_target(self) -> ThemeTarget {
        match self {
            Self::Direct(target) => target,
            Self::Textual {
                terminal_target, ..
            } => terminal_target,
        }
    }

    fn owns_target(self, target: ThemeTarget) -> bool {
        match self {
            Self::Direct(owned) => owned == target,
            Self::Textual {
                owned_rule_targets, ..
            } => owned_rule_targets.contains(&target),
        }
    }
}

/// One family-owned terminal domain reconciled only for mechanisms classified Unsupported.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UnsupportedTerminalDomain<'a> {
    resolution: TerminalStyleResolution<'a>,
    variants: TerminalVariantDomain<'a>,
    reconcile_rules: bool,
}

impl<'a> UnsupportedTerminalDomain<'a> {
    pub(crate) const fn direct(target: ThemeTarget, variants: TerminalVariantDomain<'a>) -> Self {
        Self {
            resolution: TerminalStyleResolution::Direct(target),
            variants,
            reconcile_rules: true,
        }
    }

    /// Reconciles only ordinal-palette and effect-binding fallbacks for a surface whose rule
    /// facets remain owned by a family-local plan.
    pub(crate) const fn fallbacks_only(
        target: ThemeTarget,
        variants: TerminalVariantDomain<'a>,
    ) -> Self {
        Self {
            resolution: TerminalStyleResolution::Direct(target),
            variants,
            reconcile_rules: false,
        }
    }

    pub(crate) const fn textual(
        terminal_target: ThemeTarget,
        owned_rule_targets: &'a [ThemeTarget],
        variants: TerminalVariantDomain<'a>,
    ) -> Self {
        Self {
            resolution: TerminalStyleResolution::Textual {
                terminal_target,
                owned_rule_targets,
            },
            variants,
            reconcile_rules: true,
        }
    }
}

#[derive(Debug)]
enum UnsupportedMechanismObservation {
    Rule {
        all_unsupported: bool,
        reasons_by_property: BTreeMap<ResolvedStyleProperty, FamilyThemeResidualReason>,
    },
    OrdinalPalette {
        unsupported: bool,
    },
    EffectBinding {
        unsupported: bool,
    },
}

impl UnsupportedMechanismObservation {
    fn is_owned(&self) -> bool {
        match self {
            Self::Rule {
                all_unsupported, ..
            } => *all_unsupported,
            Self::OrdinalPalette { unsupported } | Self::EffectBinding { unsupported } => {
                *unsupported
            }
        }
    }
}

/// Reconciles terminal-less or unsupported family surfaces from real family occurrence facts.
///
/// The family owns occurrence identity. This shared algorithm owns selector matching, source-order
/// winners, palette fallback, effect-binding selection, and final evidence accounting.
pub(crate) fn reconcile_unsupported_terminal_domains(
    theme: &ResolvedDiagramTheme,
    evidence: &mut FamilyThemeEvidence,
    domains: &[UnsupportedTerminalDomain<'_>],
    work_meter: &OperationWorkMeter,
) -> Result<(), OperationWorkError> {
    work_meter.charge(theme.family_mechanism_routes().len())?;

    let mut target_domains = BTreeMap::<ThemeTarget, usize>::new();
    for (domain_index, domain) in domains.iter().copied().enumerate() {
        match domain.resolution {
            TerminalStyleResolution::Direct(target) => {
                debug_assert!(target_domains.insert(target, domain_index).is_none());
            }
            TerminalStyleResolution::Textual {
                owned_rule_targets, ..
            } => {
                for target in owned_rule_targets.iter().copied() {
                    debug_assert!(target_domains.insert(target, domain_index).is_none());
                }
            }
        }
    }

    let mut observations = (0..domains.len())
        .map(|_| BTreeMap::<FamilyThemeMechanismKey, UnsupportedMechanismObservation>::new())
        .collect::<Vec<_>>();
    for route in theme.family_mechanism_routes().iter().copied() {
        let target = match route.mechanism() {
            FamilyThemeMechanism::BaseTypography(_) => continue,
            FamilyThemeMechanism::RuleFacet { target, .. }
            | FamilyThemeMechanism::OrdinalPalette { target }
            | FamilyThemeMechanism::EffectBinding { target, .. } => target,
        };
        let Some(domain_index) = target_domains.get(&target).copied() else {
            continue;
        };
        let key = theme.family_mechanism_key(route);
        match route.mechanism() {
            FamilyThemeMechanism::RuleFacet { facet, .. } => {
                if !domains[domain_index].reconcile_rules {
                    continue;
                }
                let entry = observations[domain_index].entry(key).or_insert_with(|| {
                    UnsupportedMechanismObservation::Rule {
                        all_unsupported: true,
                        reasons_by_property: BTreeMap::new(),
                    }
                });
                let UnsupportedMechanismObservation::Rule {
                    all_unsupported,
                    reasons_by_property,
                } = entry
                else {
                    unreachable!("one family mechanism key has one mechanism kind")
                };
                *all_unsupported &= route.disposition() == FamilyThemeDisposition::Unsupported;
                reasons_by_property
                    .entry(resolved_style_property_for_facet(facet))
                    .or_insert_with(|| unsupported_residual_for_facet(facet));
            }
            FamilyThemeMechanism::OrdinalPalette { .. } => {
                observations[domain_index].insert(
                    key,
                    UnsupportedMechanismObservation::OrdinalPalette {
                        unsupported: route.disposition() == FamilyThemeDisposition::Unsupported,
                    },
                );
            }
            FamilyThemeMechanism::EffectBinding { .. } => {
                observations[domain_index].insert(
                    key,
                    UnsupportedMechanismObservation::EffectBinding {
                        unsupported: route.disposition() == FamilyThemeDisposition::Unsupported,
                    },
                );
            }
            FamilyThemeMechanism::BaseTypography(_) => unreachable!("filtered above"),
        }
    }

    for (domain, observations) in domains.iter().copied().zip(observations) {
        let mut outcomes = observations
            .iter()
            .filter(|(_, observation)| observation.is_owned())
            .map(|(key, _)| (key.clone(), None))
            .collect::<BTreeMap<_, Option<FamilyThemeResidualReason>>>();
        // A domain with no owned unsupported mechanism has nothing to reconcile.  In
        // particular, do not rescan every ordinal occurrence just to discover that all
        // observations are typed, shadowed, or otherwise not owned by this fallback path.
        if outcomes.is_empty() {
            continue;
        }
        domain.variants.for_each(|ordinal, variant| {
            work_meter.charge(1)?;
            let terminal_target = domain.resolution.terminal_target();
            let style = match domain.resolution {
                TerminalStyleResolution::Direct(_) => theme.style_with_work_meter(
                    terminal_target,
                    variant,
                    Some(ordinal),
                    work_meter,
                )?,
                TerminalStyleResolution::Textual { .. } => theme.text_style_with_work_meter(
                    terminal_target,
                    variant,
                    Some(ordinal),
                    work_meter,
                )?,
            };

            for (property, origin) in style.winner_rule_properties() {
                if !domain.resolution.owns_target(origin.target()) {
                    continue;
                }
                let key = FamilyThemeMechanismKey::Rule {
                    index: origin.rule_index(),
                    target: origin.target(),
                };
                let Some(UnsupportedMechanismObservation::Rule {
                    reasons_by_property,
                    ..
                }) = observations.get(&key)
                else {
                    continue;
                };
                if let Some(reason) = reasons_by_property.get(&property).copied()
                    && let Some(outcome) = outcomes.get_mut(&key)
                {
                    outcome.get_or_insert(reason);
                }
            }

            if matches!(style.fill_resolution().specified(), Specified::Unspecified) {
                for key in observations
                    .keys()
                    .filter(|key| matches!(key, FamilyThemeMechanismKey::OrdinalPalette { .. }))
                {
                    let FamilyThemeMechanismKey::OrdinalPalette { target } = key else {
                        unreachable!("filtered palette key")
                    };
                    if theme.series_color(*target, ordinal).is_some()
                        && let Some(outcome) = outcomes.get_mut(key)
                    {
                        outcome.get_or_insert(FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    }
                }
            }

            if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
                theme.resolve_effect(terminal_target, style.effect_resolution())
            {
                let key = FamilyThemeMechanismKey::EffectBinding {
                    target: binding.target(),
                    effect_id: binding.effect_id().to_string(),
                };
                if let Some(outcome) = outcomes.get_mut(&key) {
                    outcome.get_or_insert(FamilyThemeResidualReason::UnsupportedEffect);
                }
            }
            Ok(())
        })?;

        for (key, outcome) in outcomes {
            if let Some(reason) = outcome {
                evidence.mark_residual(key, reason);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }

    Ok(())
}

pub(crate) const fn resolved_style_property_for_facet(
    facet: FamilyThemeRuleFacet,
) -> ResolvedStyleProperty {
    match facet {
        FamilyThemeRuleFacet::Fill(_) => ResolvedStyleProperty::Fill,
        FamilyThemeRuleFacet::Stroke(_) => ResolvedStyleProperty::Stroke,
        FamilyThemeRuleFacet::StrokeWidth => ResolvedStyleProperty::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray => ResolvedStyleProperty::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap => ResolvedStyleProperty::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin => ResolvedStyleProperty::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity => ResolvedStyleProperty::Opacity,
        FamilyThemeRuleFacet::FillOpacity => ResolvedStyleProperty::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity => ResolvedStyleProperty::StrokeOpacity,
        FamilyThemeRuleFacet::Radius => ResolvedStyleProperty::Radius,
        FamilyThemeRuleFacet::Padding => ResolvedStyleProperty::Padding,
        FamilyThemeRuleFacet::Typography(property) => ResolvedStyleProperty::Typography(property),
        FamilyThemeRuleFacet::Effect => ResolvedStyleProperty::Effect,
    }
}

pub(crate) const fn unsupported_residual_for_facet(
    facet: FamilyThemeRuleFacet,
) -> FamilyThemeResidualReason {
    match facet {
        FamilyThemeRuleFacet::Typography(_) => FamilyThemeResidualReason::UnsupportedTypography,
        FamilyThemeRuleFacet::Effect => FamilyThemeResidualReason::UnsupportedEffect,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => {
            FamilyThemeResidualReason::UnsupportedPaint
        }
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding => FamilyThemeResidualReason::UnsupportedGeometry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding,
        EffectGraph, EffectInput, EffectPrimitive, FamilyThemeMechanismKey, OrdinalPalette,
        OrdinalSelector, ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
        ThemeVariant,
    };
    use crate::family::FamilyThemeEvidence;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    fn resolved_theme(
        family: DiagramFamilyId,
        rules: impl IntoIterator<Item = ThemeRule>,
    ) -> crate::diagram_theme::ResolvedDiagramTheme {
        let styles = rules
            .into_iter()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile terminal evidence fixture")
            .resolve(family)
    }

    fn resolved_spec(
        family: DiagramFamilyId,
        spec: DiagramThemeSpec,
    ) -> crate::diagram_theme::ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile terminal evidence fixture")
            .resolve(family)
    }

    fn fill_rule(target: ThemeTarget, color: &str) -> ThemeRule {
        ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(color).expect("valid terminal evidence fixture color"),
            ),
        )
    }

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    #[test]
    fn unsupported_terminal_rule_is_residual_only_when_it_wins_a_real_occurrence() {
        let theme = resolved_theme(
            DiagramFamilyId::TREE_VIEW,
            [fill_rule(ThemeTarget::Node, "#123456").for_family(DiagramFamilyId::TREE_VIEW)],
        );
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        };

        let mut visible = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut visible,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile visible TreeView Node terminal");
        assert_eq!(visible.residuals().len(), 1);
        assert_eq!(visible.residuals()[0].key(), &key);
        assert!(visible.not_applicable_mechanisms().is_empty());

        let mut absent = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut absent,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(0, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile absent TreeView Node terminal");
        assert_eq!(absent.not_applicable_mechanisms(), &[key]);
        assert!(absent.residuals().is_empty());
    }

    #[test]
    fn unsupported_terminal_rules_respect_shadowing_and_ordinal_intersection() {
        let theme = resolved_theme(
            DiagramFamilyId::TREE_VIEW,
            [
                fill_rule(ThemeTarget::Node, "#111111").for_family(DiagramFamilyId::TREE_VIEW),
                fill_rule(ThemeTarget::Node, "#222222").for_family(DiagramFamilyId::TREE_VIEW),
                fill_rule(ThemeTarget::Node, "#333333")
                    .for_family(DiagramFamilyId::TREE_VIEW)
                    .with_ordinal(OrdinalSelector::exact(2).expect("valid exact ordinal")),
            ],
        );
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));

        reconcile_unsupported_terminal_domains(
            &theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile shadowed and non-intersecting TreeView rules");

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 2,
                    target: ThemeTarget::Node,
                },
            ]
        );
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::Node,
            }
        );
    }

    #[test]
    fn domains_without_owned_outcomes_do_not_resolve_each_occurrence() {
        let theme = resolved_theme(
            DiagramFamilyId::TREE_VIEW,
            [fill_rule(ThemeTarget::Edge, "#123456").for_family(DiagramFamilyId::TREE_VIEW)],
        );
        let route_count = theme.family_mechanism_routes().len();
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, route_count)
            .expect("exact route-scan work limit");
        let meter = OperationWorkMeter::new(policy);
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));

        reconcile_unsupported_terminal_domains(
            &theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(10_000, ThemeVariant::Default),
            )],
            &meter,
        )
        .expect("a domain with no owned fallback must stop after the route scan");
    }

    #[test]
    fn textual_terminal_domain_uses_one_text_and_role_local_winner_chain() {
        let theme = resolved_theme(
            DiagramFamilyId::MINDMAP,
            [
                fill_rule(ThemeTarget::Text, "#111111").for_family(DiagramFamilyId::MINDMAP),
                fill_rule(ThemeTarget::NodeLabel, "#222222").for_family(DiagramFamilyId::MINDMAP),
            ],
        );
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));

        reconcile_unsupported_terminal_domains(
            &theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::textual(
                ThemeTarget::NodeLabel,
                &[ThemeTarget::Text, ThemeTarget::NodeLabel],
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile shared Mindmap label terminal");

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            }]
        );
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            }
        );
    }

    #[test]
    fn unsupported_ordinal_palette_tracks_visible_and_absent_occurrences() {
        let palette =
            OrdinalPalette::new([ThemeColorValue::parse("#123456").expect("valid palette color")])
                .expect("non-empty palette");
        let theme = resolved_spec(
            DiagramFamilyId::TREE_VIEW,
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ),
        );
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };

        let mut visible = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut visible,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile visible unsupported palette");
        assert_eq!(visible.residuals().len(), 1);
        assert_eq!(visible.residuals()[0].key(), &key);
        assert_eq!(
            visible.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
        );

        let mut absent = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut absent,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(0, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile absent unsupported palette");
        assert_eq!(absent.not_applicable_mechanisms(), &[key]);
        assert!(absent.residuals().is_empty());
    }

    #[test]
    fn fallback_only_palette_uses_the_real_fill_winner_without_claiming_the_rule() {
        let palette =
            OrdinalPalette::new([ThemeColorValue::parse("#123456").expect("valid palette color")])
                .expect("non-empty palette");
        let theme = resolved_spec(
            DiagramFamilyId::TREE_VIEW,
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        fill_rule(ThemeTarget::Edge, "#abcdef")
                            .for_family(DiagramFamilyId::TREE_VIEW),
                    )
                    .with_ordinal_palette(ThemeTarget::Edge, palette),
            ),
        );
        let rule_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        };
        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Edge,
        };
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));

        reconcile_unsupported_terminal_domains(
            &theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile only TreeView Edge fallbacks");

        assert_eq!(evidence.not_applicable_mechanisms(), &[palette_key]);
        assert!(evidence.residuals().is_empty());
        assert!(!evidence.not_applicable_mechanisms().contains(&rule_key));
    }

    #[test]
    fn fallback_only_effect_binding_yields_to_an_explicit_effect_winner() {
        let bound_effect_id = "tree-edge-bound";
        let explicit_effect_id = "tree-edge-explicit";
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    bound_effect_id,
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    }],
                )
                .expect("valid bound effect graph"),
            )
            .expect("unique bound effect graph")
            .with_graph(
                EffectGraph::new(
                    explicit_effect_id,
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 2.0,
                    }],
                )
                .expect("valid explicit effect graph"),
            )
            .expect("unique explicit effect graph")
            .with_binding(
                EffectBinding::new(ThemeTarget::Edge, bound_effect_id)
                    .expect("valid effect binding"),
            )
            .expect("unique effect binding");
        let explicit_rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_effect(explicit_effect_id)
                .expect("valid explicit effect"),
        )
        .for_family(DiagramFamilyId::TREE_VIEW);
        let theme = resolved_spec(
            DiagramFamilyId::TREE_VIEW,
            DiagramThemeSpec::new()
                .with_styles(ThemeRuleSet::default().with_rule(explicit_rule))
                .with_effects(effects),
        );
        let rule_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Edge,
        };
        let binding_key = FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::Edge,
            effect_id: bound_effect_id.to_string(),
        };
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));

        reconcile_unsupported_terminal_domains(
            &theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile only TreeView Edge fallbacks");

        assert_eq!(evidence.not_applicable_mechanisms(), &[binding_key]);
        assert!(evidence.residuals().is_empty());
        assert!(!evidence.not_applicable_mechanisms().contains(&rule_key));
    }

    #[test]
    fn unsupported_effect_binding_tracks_visible_and_absent_occurrences() {
        let effect_id = "tree-node-blur";
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    effect_id,
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    }],
                )
                .expect("valid effect graph"),
            )
            .expect("unique effect graph")
            .with_binding(
                EffectBinding::new(ThemeTarget::Node, effect_id).expect("valid effect binding"),
            )
            .expect("unique effect binding");
        let theme = resolved_spec(
            DiagramFamilyId::TREE_VIEW,
            DiagramThemeSpec::new().with_effects(effects),
        );
        let key = FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::Node,
            effect_id: effect_id.to_string(),
        };

        let mut visible = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut visible,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(1, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile visible unsupported effect");
        assert_eq!(visible.residuals().len(), 1);
        assert_eq!(visible.residuals()[0].key(), &key);
        assert_eq!(
            visible.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedEffect,
        );

        let mut absent = FamilyThemeEvidence::from_theme(Some(&theme));
        reconcile_unsupported_terminal_domains(
            &theme,
            &mut absent,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(0, ThemeVariant::Default),
            )],
            &work_meter(),
        )
        .expect("reconcile absent unsupported effect");
        assert_eq!(absent.not_applicable_mechanisms(), &[key]);
        assert!(absent.residuals().is_empty());
    }
}
