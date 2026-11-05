use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::resources::OperationWorkMeter;

/// Resolution and accounting for one default-variant title with a static fill.
/// Each family retains its own source ownership and concrete writer receipt checks.
#[derive(Debug, Default)]
pub(crate) struct SingleTitleFillPlan {
    fill_css: Option<Box<str>>,
    typed_fill_capability: Option<ThemeCapability>,
    evidence: FamilyThemeOutcomes,
    pending_fill_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl SingleTitleFillPlan {
    pub(crate) fn resolve(
        theme: &ResolvedDiagramTheme,
        title_present: bool,
        config_owns_fill: bool,
        mut evidence: FamilyThemeOutcomes,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_count = usize::from(title_present);
        let style = title_present.then(|| {
            theme.style_with_work_meter(
                ThemeTarget::Title,
                ThemeVariant::Default,
                Some(1),
                work_meter,
            )
        });
        let style = style.transpose()?;
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
                        &[ThemeTarget::Title],
                        DirectStaticSelectorDomain::Default,
                    )
                })
            })
            .flatten();
        let (fill_css, typed_fill_rule, typed_fill_capability) =
            typed_fill.map_or((None, None, None), |fill| {
                let (css, rule_index, capability) = fill.into_parts();
                (Some(css), Some(rule_index), Some(capability))
            });
        let source_owned_fill = [config_owns_fill];

        let mut observations = BTreeMap::<usize, TitleRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Title,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(title_count) {
                        continue;
                    }
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    if !route_won {
                        continue;
                    }

                    observation.applicable = true;
                    if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        continue;
                    }
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                            FamilyThemeRuleFacet::Fill(_),
                        ) if typed_fill_rule == Some(rule_index) => {
                            observation.fill_pending = true;
                        }
                        (_, _, FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Clear)) => {
                            // Clear needs a family-owned baseline and a matching writer receipt.
                            observation
                                .residual
                                .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                } => {
                    // Reconciled below from the final title fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Reconciled below from the final title style.
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
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending_fill_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Title,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules stay unaccounted until every winning facet has a terminal owner.
            } else if observation.fill_pending {
                debug_assert!(pending_fill_key.is_none());
                pending_fill_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            fill_css,
            typed_fill_capability,
            evidence,
            pending_fill_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css.as_deref()
    }

    /// Called only after the family-specific writer receipt proves the selected fill.
    pub(crate) fn record_proven_terminal(&self) -> bool {
        self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        if let Some(key) = self.pending_fill_key.clone()
            && self.terminal_receipt.get().is_some()
            && let Some(capability) = self.typed_fill_capability
        {
            evidence.mark_applied_with_capabilities(key, [capability]);
        }
    }
}

#[derive(Debug, Default)]
struct TitleRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, Specified, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn resolve(
        styles: ThemeRuleSet,
        present: bool,
        owned: bool,
    ) -> (ResolvedDiagramTheme, SingleTitleFillPlan) {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap()
            .resolve(crate::DiagramFamilyId::TREEMAP);
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = SingleTitleFillPlan::resolve(
            &theme,
            present,
            owned,
            FamilyThemeOutcomes::default(),
            &work,
        )
        .unwrap();
        (theme, plan)
    }

    #[test]
    fn variants_without_a_default_title_winner_are_not_applicable() {
        for variant in [ThemeVariant::Primary, ThemeVariant::Error] {
            for ordinal in [None, Some(OrdinalSelector::Exact(1))] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                )
                .with_variant(variant);
                if let Some(ordinal) = ordinal {
                    rule = rule.with_ordinal(ordinal);
                }
                let (theme, plan) = resolve(ThemeRuleSet::default().with_rule(rule), true, false);
                assert_eq!(plan.fill_css(), None);
                let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));
                plan.record_evidence(&mut evidence);
                assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
                assert!(evidence.applied().is_empty());
                assert!(evidence.residuals().is_empty());
            }
        }
    }

    #[test]
    fn static_default_selectors_require_the_family_terminal_proof() {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, css) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let (theme, plan) = resolve(ThemeRuleSet::default().with_rule(rule), true, false);
                assert_eq!(plan.fill_css(), Some(css));
                let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));
                plan.record_evidence(&mut evidence);
                assert!(evidence.applied().is_empty());
                assert!(plan.record_proven_terminal());
                assert!(!plan.record_proven_terminal());
                plan.record_evidence(&mut evidence);
                assert_eq!(evidence.applied().len(), 1);
                assert!(evidence.residuals().is_empty());
            }
        }
    }

    #[test]
    fn clear_is_residual_only_for_a_visible_unowned_title() {
        for (present, owned, residual) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = Specified::Clear;
            let (theme, plan) = resolve(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Title, patch)),
                present,
                owned,
            );
            assert_eq!(plan.fill_css(), None);
            assert!(plan.record_proven_terminal());
            let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));
            plan.record_evidence(&mut evidence);
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.residuals().len(), usize::from(residual));
            assert_eq!(
                evidence.not_applicable_mechanisms().len(),
                usize::from(!residual)
            );
        }
    }
}
