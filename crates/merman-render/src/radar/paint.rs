use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedProperty, ResolvedStyleProperty,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, FamilyThemeEvidence, FamilyThemeOutcomes, FamilyThemeResidualReason,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// The bounded scalar paint domain shared by Radar's series and legend label writers.
pub(super) fn static_paint(
    theme: &ResolvedDiagramTheme,
    resolution: &ResolvedProperty<CanvasPaint>,
    target: ThemeTarget,
    property: ResolvedStyleProperty,
    baseline: &str,
) -> Option<DirectPaintExpectation> {
    let origin = resolution.winner()?;
    let facet = match property {
        ResolvedStyleProperty::Fill => FamilyThemeRuleFacet::fill(resolution.specified())?,
        ResolvedStyleProperty::Stroke => FamilyThemeRuleFacet::stroke(resolution.specified())?,
        _ => return None,
    };
    if origin.target() != target
        || origin.ordinal().is_some()
        || !matches!(origin.variant(), None | Some(ThemeVariant::Default))
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let (css, capability) = match resolution.specified() {
        Specified::Clear => (baseline.to_owned(), ThemeCapability::SemanticRules),
        Specified::Value(CanvasPaint::Transparent) => {
            ("transparent".to_owned(), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => (
            color.as_css(),
            if color.is_transparent() {
                ThemeCapability::TransparentPaint
            } else {
                ThemeCapability::SolidPaint
            },
        ),
        _ => return None,
    };
    Some(DirectPaintExpectation::new(
        origin.rule_index(),
        css,
        capability,
    ))
}

#[derive(Debug, Default)]
struct RuleObservation {
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

/// Rule outcomes remain per rule, while winners and source ownership remain per property.
#[derive(Debug)]
pub(super) struct PaintRules {
    target: ThemeTarget,
    facets: BTreeMap<(usize, ResolvedStyleProperty), FamilyThemeRuleFacet>,
    observations: BTreeMap<usize, RuleObservation>,
}

impl PaintRules {
    pub(super) fn new(theme: &ResolvedDiagramTheme, target: ThemeTarget) -> Self {
        let mut facets = BTreeMap::new();
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            if let FamilyThemeMechanism::RuleFacet {
                target: candidate,
                rule_index,
                facet,
                ..
            } = route.mechanism()
                && candidate == target
            {
                facets.insert(
                    (rule_index, resolved_style_property_for_facet(facet)),
                    facet,
                );
                observations.entry(rule_index).or_default();
            }
        }
        Self {
            target,
            facets,
            observations,
        }
    }

    pub(super) fn observe(
        &mut self,
        style: &ResolvedThemeStyle,
        source_owned: [bool; 2],
        paints: [Option<&DirectPaintExpectation>; 2],
        visibility_known: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        work_meter.charge(self.facets.len())?;
        for (property, origin) in style.winner_rule_properties() {
            let Some(facet) = self.facets.get(&(origin.rule_index(), property)) else {
                continue;
            };
            let paint_index = match property {
                ResolvedStyleProperty::Fill => Some(0),
                ResolvedStyleProperty::Stroke => Some(1),
                _ => None,
            };
            if paint_index.is_some_and(|index| source_owned[index]) {
                continue;
            }
            let observation = self
                .observations
                .get_mut(&origin.rule_index())
                .expect("registered Radar rule");
            if !visibility_known {
                observation.residual = Some(FamilyThemeResidualReason::UnsupportedTypography);
                continue;
            }
            if let Some(paint) = paint_index.and_then(|index| paints[index])
                && paint.rule_index() == origin.rule_index()
            {
                observation.capabilities.insert(paint.capability());
            } else {
                observation
                    .residual
                    .get_or_insert(unsupported_residual_for_facet(*facet));
            }
        }
        Ok(())
    }

    pub(super) fn finish(self, evidence: &mut FamilyThemeOutcomes) -> PendingPaintRules {
        let mut pending = BTreeMap::new();
        for (index, observation) in self.observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: self.target,
            };
            if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                pending.insert(key, observation.capabilities);
            }
        }
        pending
    }
}

pub(super) type PendingPaintRules = BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>;

pub(super) fn record_pending(
    pending: &PendingPaintRules,
    terminal_complete: bool,
    evidence: &mut FamilyThemeEvidence,
) {
    for (key, capabilities) in pending {
        if terminal_complete {
            evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
        } else {
            evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
        }
    }
}
