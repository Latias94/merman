use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, OrdinalSelector,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeOutcomes, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod edge_stroke;
mod surfaces;
mod typography;

use edge_stroke::ArchitectureEdgeStrokePlan;
use surfaces::ArchitectureSurfaceThemePlan;
pub(crate) use surfaces::{
    ArchitectureArrowSide, ArchitectureEdgeTerminal, ArchitectureEdgeTerminalEmission,
    ArchitectureGroupTerminal, ArchitectureGroupTerminalEmission,
    ArchitecturePaintTerminalEmission, ArchitectureServiceTerminal,
    ArchitectureServiceTerminalEmission, ArchitectureSurfaceRenderState,
    ArchitectureTextTerminalEmission,
};
pub(crate) use typography::{ArchitectureTypographyEmission, ArchitectureTypographyThemePlan};

/// Final Architecture group paint shared by terminal emission and family evidence.
#[derive(Debug)]
pub(crate) struct ArchitectureGroupThemePlan {
    inline_style: Option<String>,
    evidence: FamilyThemeOutcomes,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    edge_stroke: ArchitectureEdgeStrokePlan,
    surfaces: Box<ArchitectureSurfaceThemePlan>,
    typography: ArchitectureTypographyThemePlan,
}

impl ArchitectureGroupThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        group_count: usize,
        edge_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let typography = ArchitectureTypographyThemePlan::resolve(theme, effective_config);
        let Some(theme) = theme else {
            return Ok(Self::baseline(typography));
        };
        let mut evidence = FamilyThemeOutcomes::default();
        let surfaces = Box::new(ArchitectureSurfaceThemePlan::resolve(
            Some(theme),
            effective_config,
        ));
        let edge_stroke = ArchitectureEdgeStrokePlan::resolve(
            theme,
            effective_config,
            edge_count,
            work_meter,
            &mut evidence,
        )?;
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
                edge_stroke,
                surfaces,
                typography,
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
                            FamilyThemeDisposition::TypedAdapter => {
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
                        FamilyThemeDisposition::TypedAdapter => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Cluster,
                } => {
                    // Unsupported fallback mechanisms are reconciled from the actual
                    // per-group resolution below. A static occurrence count is not enough:
                    // an explicit typed Cluster.fill winner suppresses the palette.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Cluster,
                    ..
                } => {
                    // See the fallback reconciliation below; only an effect that remains
                    // selected by a real group terminal is residual.
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

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Cluster,
                TerminalVariantDomain::uniform(group_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

        Ok(Self {
            inline_style,
            evidence,
            pending,
            edge_stroke,
            surfaces,
            typography,
        })
    }

    fn baseline(typography: ArchitectureTypographyThemePlan) -> Self {
        Self {
            inline_style: None,
            evidence: FamilyThemeOutcomes::default(),
            pending: BTreeMap::new(),
            edge_stroke: ArchitectureEdgeStrokePlan::baseline(),
            surfaces: Box::new(ArchitectureSurfaceThemePlan::baseline()),
            typography,
        }
    }

    pub(crate) fn inline_style(&self) -> Option<&str> {
        self.inline_style.as_deref()
    }

    pub(crate) fn edge_stroke(&self) -> Option<&str> {
        self.edge_stroke.terminal_stroke()
    }

    pub(crate) const fn typography_theme(&self) -> &ArchitectureTypographyThemePlan {
        &self.typography
    }

    pub(crate) fn prepare_surface_styles(
        &self,
        services: impl IntoIterator<Item = ArchitectureServiceTerminal>,
        groups: impl IntoIterator<Item = ArchitectureGroupTerminal>,
        edges: impl IntoIterator<Item = ArchitectureEdgeTerminal>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Option<ArchitectureSurfaceRenderState>, OperationWorkError> {
        self.surfaces
            .prepare_styles(services, groups, edges, work_meter)
    }

    pub(crate) fn finish_evidence(
        &self,
        typography: Option<ArchitectureTypographyEmission>,
        group_style_emitted: bool,
        edge_style_emitted: bool,
        surfaces: Option<&ArchitectureSurfaceRenderState>,
    ) -> FamilyThemeOutcomes {
        let mut evidence = self.evidence.clone();
        evidence.merge_accounted_from(self.typography.finish_evidence(typography));
        if group_style_emitted {
            for (key, capabilities) in &self.pending {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            }
        }
        self.edge_stroke
            .finish_evidence(&mut evidence, edge_style_emitted);
        self.surfaces.finish_evidence(&mut evidence, surfaces);
        evidence
    }
}

/// Theme evidence returned with the completed Architecture SVG.
pub(crate) struct ArchitectureThemeOutcome {
    pub(crate) evidence: FamilyThemeOutcomes,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) text_receipt: Option<crate::__private::ArchitectureTextReceipt>,
}

fn mermaid_owns_group_stroke(effective_config: &merman_core::MermaidConfig) -> bool {
    if merman_core::__private::config_paint_overrides_typed_default(
        effective_config,
        "themeVariables.archGroupBorderColor",
    ) {
        return true;
    }
    merman_core::__private::config_paint_overrides_typed_default(
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
