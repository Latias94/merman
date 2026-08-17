use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_BOUNDARY_RADIUS_PX: f64 = 2.5;

/// Final C4 explicit-boundary radius shared by terminal SVG emission and family evidence.
#[derive(Debug)]
pub(crate) struct C4ClusterThemePlan {
    explicit_boundary_count: usize,
    radius_token: Box<str>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl C4ClusterThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        explicit_boundary_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(explicit_boundary_count));
        };

        let static_style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winners = static_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let static_radius_candidate = typed_radius_token(theme, &static_style);
        let direct_static_winner_rule = static_style
            .radius_resolution()
            .winner()
            .filter(|origin| {
                theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
                    == Some(FamilyThemeDisposition::TypedAdapter)
            })
            .map(|origin| origin.rule_index());

        let has_ordinal_cluster_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Cluster && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut static_radius_wins_every_boundary =
            explicit_boundary_count != 0 && direct_static_winner_rule.is_some();
        if explicit_boundary_count != 0 {
            if has_ordinal_cluster_rules {
                for ordinal in 1..=explicit_boundary_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Cluster,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    occurrence_winners.extend(
                        style
                            .winner_rule_properties()
                            .into_iter()
                            .map(|(property, origin)| (origin.rule_index(), property)),
                    );
                    static_radius_wins_every_boundary &= style
                        .radius_resolution()
                        .winner()
                        .map(|origin| origin.rule_index())
                        == direct_static_winner_rule;
                }
            } else {
                occurrence_winners.extend(static_winners.iter().copied());
            }
        }
        let terminal_winner_rule = static_radius_wins_every_boundary
            .then_some(direct_static_winner_rule)
            .flatten();
        let radius_token = terminal_winner_rule
            .and(static_radius_candidate)
            .unwrap_or_else(baseline_radius_token);

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, C4ClusterRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Cluster,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(explicit_boundary_count)
                    {
                        continue;
                    }

                    let property = resolved_style_property_for_facet(facet);
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    let route_won = match selector {
                        FamilyThemeSelectorShape::Static { variant: None } => {
                            static_winners.contains(&(rule_index, property))
                        }
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                        | FamilyThemeSelectorShape::Ordinal { .. } => {
                            occurrence_winners.contains(&(rule_index, property))
                        }
                    };
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Radius,
                        ) if terminal_winner_rule == Some(rule_index) => {
                            observation.radius_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Cluster,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if explicit_boundary_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Cluster,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if explicit_boundary_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let mut pending_radius_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Cluster,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule stays unaccounted until every winning facet has a terminal owner.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            explicit_boundary_count,
            radius_token,
            evidence,
            pending_radius_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(explicit_boundary_count: usize) -> Self {
        Self {
            explicit_boundary_count,
            radius_token: baseline_radius_token(),
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn radius_token(&self) -> &str {
        &self.radius_token
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4ClusterRadiusThemeReceipt> {
        self.pending_radius_key.as_ref().map(|_| {
            C4ClusterRadiusThemeReceipt::new(
                self.explicit_boundary_count,
                self.radius_token.clone(),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: C4ClusterRadiusThemeReceipt) -> bool {
        self.pending_radius_key.is_some()
            && receipt.proves(self.explicit_boundary_count)
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_radius_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
        }
        evidence
    }
}

fn baseline_radius_token() -> Box<str> {
    MERMAID_BOUNDARY_RADIUS_PX.to_string().into_boxed_str()
}

fn typed_radius_token(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<Box<str>> {
    let origin = style.radius_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.radius_resolution().specified() {
        Specified::Value(value) => Some(value.to_string().into_boxed_str()),
        Specified::Clear => Some(baseline_radius_token()),
        Specified::Unspecified => None,
    }
}

/// Writer-owned proof that every explicit C4 boundary emitted the resolved radius once.
#[derive(Debug)]
pub(crate) struct C4ClusterRadiusThemeReceipt {
    expected_boundary_count: usize,
    expected_radius_token: Box<str>,
    next_boundary_ordinal: usize,
    attributes_match: bool,
}

impl C4ClusterRadiusThemeReceipt {
    fn new(expected_boundary_count: usize, expected_radius_token: Box<str>) -> Self {
        Self {
            expected_boundary_count,
            expected_radius_token,
            next_boundary_ordinal: 0,
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_boundary(
        &mut self,
        boundary_ordinal: usize,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
    ) {
        if boundary_ordinal != self.next_boundary_ordinal {
            self.attributes_match = false;
            return;
        }
        self.next_boundary_ordinal = self.next_boundary_ordinal.saturating_add(1);
        self.attributes_match &= emitted_rx_token == self.expected_radius_token.as_ref()
            && emitted_ry_token == self.expected_radius_token.as_ref();
    }

    fn proves(&self, expected_boundary_count: usize) -> bool {
        self.expected_boundary_count == expected_boundary_count
            && self.next_boundary_ordinal == expected_boundary_count
            && self.attributes_match
    }
}

#[derive(Debug, Default)]
struct C4ClusterRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}
