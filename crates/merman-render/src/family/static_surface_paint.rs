use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeMechanism, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Static paints for an existing family surface, with independent source ownership per property.
#[derive(Debug, Default)]
pub(crate) struct StaticSurfacePaint {
    pub(crate) fill: Option<DirectStaticPaint>,
    pub(crate) stroke: Option<DirectStaticPaint>,
    evidence: FamilyThemeOutcomes,
    fallback: FamilyThemeOutcomes,
}

impl StaticSurfacePaint {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        target: ThemeTarget,
        source_fill: &[bool],
        source_stroke: &[bool],
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        debug_assert_eq!(source_fill.len(), source_stroke.len());
        let style = theme.style_with_work_meter(target, ThemeVariant::Default, None, work)?;
        let fill_available = source_fill.iter().any(|&owned| !owned);
        let stroke_available = source_stroke.iter().any(|&owned| !owned);
        let mut winner_properties = BTreeSet::new();
        for (index, (&fill_owned, &stroke_owned)) in
            source_fill.iter().zip(source_stroke).enumerate()
        {
            let occurrence = theme.style_with_work_meter(
                target,
                ThemeVariant::Default,
                Some(index + 1),
                work,
            )?;
            winner_properties.extend(occurrence.winner_rule_properties().filter_map(
                |(property, origin)| {
                    let source_owned = match property {
                        ResolvedStyleProperty::Fill => fill_owned,
                        ResolvedStyleProperty::Stroke => stroke_owned,
                        _ => false,
                    };
                    (!source_owned).then_some((origin.rule_index(), property))
                },
            ));
        }
        let fill = fill_available
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[target],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let stroke = stroke_available
            .then(|| {
                resolve_direct_static_stroke(
                    theme,
                    &style,
                    &[target],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let mut fallback = FamilyThemeOutcomes::default();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut fallback,
            &[UnsupportedTerminalDomain::direct(
                target,
                TerminalVariantDomain::uniform(source_fill.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(source_fill)
            .with_source_owned_stroke(source_stroke)],
            work,
        )?;
        let mut rules = BTreeSet::new();
        let mut incomplete = BTreeSet::new();
        let mut capabilities = BTreeMap::<usize, BTreeSet<ThemeCapability>>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: rule_target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if rule_target != target {
                continue;
            }
            rules.insert(rule_index);
            let property = resolved_style_property_for_facet(facet);
            if !winner_properties.contains(&(rule_index, property)) {
                continue;
            }
            let paint = match property {
                ResolvedStyleProperty::Fill => fill.as_ref(),
                ResolvedStyleProperty::Stroke => stroke.as_ref(),
                _ => None,
            }
            .filter(|paint| paint.rule_index() == rule_index);
            if let Some(paint) = paint {
                capabilities
                    .entry(rule_index)
                    .or_default()
                    .insert(paint.capability());
            } else {
                incomplete.insert(rule_index);
            }
        }
        let mut evidence = FamilyThemeOutcomes::default();
        for index in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule { index, target };
            if incomplete.contains(&index) {
                continue;
            }
            if let Some(capabilities) = capabilities.remove(&index) {
                evidence.mark_applied_with_capabilities(key, capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        Ok(Self {
            fill,
            stroke,
            evidence,
            fallback,
        })
    }

    /// The caller records this only after the SVG writer completes the matching real surface.
    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        // A surviving unsupported sibling invalidates the whole rule. A fallback N/A must
        // wait until typed consumers have accounted for supported siblings of the same key.
        for residual in self.fallback.residuals() {
            evidence.mark_residual(residual.key().clone(), residual.reason());
        }
        evidence.merge_borrowed(&self.evidence);
        for key in self.fallback.not_applicable_mechanisms() {
            evidence.mark_not_applicable(key.clone());
        }
    }
}
