use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, OrdinalSelector,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Final Architecture group paint shared by terminal emission and family evidence.
#[derive(Debug)]
pub(crate) struct ArchitectureGroupThemePlan {
    inline_style: Option<String>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
}

impl ArchitectureGroupThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        group_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut pending = BTreeMap::new();
        if group_count == 0 {
            for route in theme.family_mechanism_routes().iter().copied() {
                if mechanism_targets_cluster(route.mechanism()) {
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
                }
            }
            return Ok(Self {
                inline_style: None,
                evidence,
                pending,
            });
        }

        let style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mermaid_owns_stroke = mermaid_owns_group_stroke(effective_config);
        let fill = typed_paint_css(theme, style.fill_resolution(), FamilyThemeRuleFacet::fill);
        let stroke = (!mermaid_owns_stroke)
            .then(|| {
                typed_paint_css(
                    theme,
                    style.stroke_resolution(),
                    FamilyThemeRuleFacet::stroke,
                )
            })
            .flatten();
        let inline_style = compose_inline_style(fill.as_deref(), stroke.as_deref());

        let winners = style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        let mut observations = BTreeMap::<usize, ArchitectureGroupRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Cluster,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector_matches_any_group(selector, group_count) {
                        continue;
                    }
                    if !matches!(
                        selector,
                        FamilyThemeSelectorShape::Static {
                            variant: None | Some(ThemeVariant::Default)
                        }
                    ) {
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
                    let source_owned =
                        property == ResolvedStyleProperty::Stroke && mermaid_owns_stroke;
                    if source_owned || winners.get(&property).copied() != Some(rule_index) {
                        continue;
                    }
                    observation.applicable = true;
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
                            ) =>
                        {
                            observation.pending = true;
                            add_paint_capabilities(&mut observation.capabilities, facet);
                        }
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
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Cluster,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if route.disposition() == FamilyThemeDisposition::Unsupported {
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
                    if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

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
                // Strict mode remains fail-closed until every winning facet has one owner.
            } else if observation.pending {
                pending.insert(key, observation.capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            inline_style,
            evidence,
            pending,
        })
    }

    fn baseline() -> Self {
        Self {
            inline_style: None,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
        }
    }

    pub(crate) fn inline_style(&self) -> Option<&str> {
        self.inline_style.as_deref()
    }

    /// Returns terminal evidence after the caller has completed the Architecture SVG.
    pub(crate) fn finish_evidence_after_svg(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for (key, capabilities) in &self.pending {
            evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
        }
        evidence
    }
}

fn mermaid_owns_group_stroke(effective_config: &merman_core::MermaidConfig) -> bool {
    if merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.archGroupBorderColor",
    ) {
        return true;
    }
    merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.primaryBorderColor",
    ) && effective_config.get_str("themeVariables.archGroupBorderColor")
        == effective_config.get_str("themeVariables.primaryBorderColor")
}

fn selector_matches_any_group(selector: FamilyThemeSelectorShape, group_count: usize) -> bool {
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        } => group_count > 0,
        FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            selector: OrdinalSelector::Exact(index),
        } => index <= group_count,
        FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            selector: OrdinalSelector::Cycle { offset, .. },
        } => offset < group_count,
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn mechanism_targets_cluster(mechanism: FamilyThemeMechanism) -> bool {
    matches!(
        mechanism,
        FamilyThemeMechanism::RuleFacet {
            target: ThemeTarget::Cluster,
            ..
        } | FamilyThemeMechanism::OrdinalPalette {
            target: ThemeTarget::Cluster,
        } | FamilyThemeMechanism::EffectBinding {
            target: ThemeTarget::Cluster,
            ..
        }
    )
}

fn typed_paint_css(
    theme: &ResolvedDiagramTheme,
    resolution: &crate::diagram_theme::ResolvedProperty<CanvasPaint>,
    facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
) -> Option<String> {
    let origin = resolution.winner()?;
    let specified = resolution.specified();
    let facet = facet(specified)?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match specified {
        Specified::Unspecified => None,
        Specified::Clear => None,
        Specified::Value(CanvasPaint::Transparent) => Some("transparent".to_owned()),
        Specified::Value(CanvasPaint::Solid(color)) => Some(color.as_css()),
        Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => None,
    }
}

fn compose_inline_style(fill: Option<&str>, stroke: Option<&str>) -> Option<String> {
    let mut style = String::new();
    if let Some(fill) = fill {
        style.push_str("fill:");
        style.push_str(fill);
        style.push(';');
    }
    if let Some(stroke) = stroke {
        style.push_str("stroke:");
        style.push_str(stroke);
        style.push(';');
    }
    (!style.is_empty()).then_some(style)
}

fn add_paint_capabilities(
    capabilities: &mut BTreeSet<ThemeCapability>,
    facet: FamilyThemeRuleFacet,
) {
    let kind = match facet {
        FamilyThemeRuleFacet::Fill(kind) | FamilyThemeRuleFacet::Stroke(kind) => kind,
        _ => return,
    };
    match kind {
        FamilyThemePaintKind::Transparent => {
            capabilities.insert(ThemeCapability::TransparentPaint);
        }
        FamilyThemePaintKind::Solid => {
            capabilities.insert(ThemeCapability::SolidPaint);
        }
        FamilyThemePaintKind::Clear
        | FamilyThemePaintKind::LinearGradient
        | FamilyThemePaintKind::RadialGradient
        | FamilyThemePaintKind::Pattern => {}
    }
    if matches!(facet, FamilyThemeRuleFacet::Stroke(_)) {
        capabilities.insert(ThemeCapability::BorderStyling);
    }
}

#[derive(Debug, Default)]
struct ArchitectureGroupRuleObservation {
    applicable: bool,
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}
