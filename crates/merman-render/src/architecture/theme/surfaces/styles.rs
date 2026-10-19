use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeEffect, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
#[cfg(merman_internal_theme_acceptance)]
use crate::model::Bounds;
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureServiceTerminal {
    pub(crate) index: usize,
    pub(crate) has_background: bool,
    pub(crate) has_title: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureGroupTerminal {
    pub(crate) index: usize,
    pub(crate) has_title: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureEdgeTerminal {
    pub(crate) index: usize,
    pub(crate) has_label: bool,
    pub(crate) has_lhs_arrow: bool,
    pub(crate) has_rhs_arrow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArchitectureArrowSide {
    Lhs,
    Rhs,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitecturePaintTerminalEmission {
    pub(crate) emitted: bool,
}

#[derive(Debug)]
pub(crate) struct ArchitectureTextTerminalEmission {
    pub(crate) emitted: bool,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) bounds: Option<Bounds>,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) fragment_digest: Option<[u8; 32]>,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) run_count: usize,
}

#[derive(Debug)]
pub(crate) struct ArchitectureServiceTerminalEmission {
    pub(crate) index: usize,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) id: Box<str>,
    pub(crate) background: ArchitecturePaintTerminalEmission,
    pub(crate) title: ArchitectureTextTerminalEmission,
}

#[derive(Debug)]
pub(crate) struct ArchitectureGroupTerminalEmission {
    pub(crate) index: usize,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) id: Box<str>,
    pub(crate) title: ArchitectureTextTerminalEmission,
}

#[derive(Debug)]
pub(crate) struct ArchitectureEdgeTerminalEmission {
    pub(crate) index: usize,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) lhs_id: Box<str>,
    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) rhs_id: Box<str>,
    pub(crate) label: ArchitectureTextTerminalEmission,
    pub(crate) lhs_arrow: ArchitecturePaintTerminalEmission,
    pub(crate) rhs_arrow: ArchitecturePaintTerminalEmission,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArchitectureSurfaceOwnership {
    pub(super) service_stroke: bool,
    pub(super) text_fill: bool,
    pub(super) arrow_fill: bool,
}

struct ArchitectureTextResolution<'a> {
    theme: &'a ResolvedDiagramTheme,
    source_owned: bool,
    ordinal: &'a mut usize,
    work_meter: &'a OperationWorkMeter,
    mechanisms: &'a mut ArchitectureTerminalMechanisms,
}

#[derive(Default)]
struct ArchitectureTerminalMechanisms {
    winner_properties: BTreeSet<(FamilyThemeMechanismKey, ResolvedStyleProperty)>,
    active_palettes: BTreeSet<FamilyThemeMechanismKey>,
    active_effects: BTreeSet<FamilyThemeMechanismKey>,
}

#[derive(Debug, Clone)]
struct ArchitectureServiceSurfaceStyle {
    #[cfg(merman_internal_theme_acceptance)]
    id: Box<str>,
    background_style: Option<Box<str>>,
    background_fill: Option<FamilyThemeMechanismKey>,
    background_stroke: Option<FamilyThemeMechanismKey>,
    title_style: Option<Box<str>>,
    title_fill: Option<FamilyThemeMechanismKey>,
    #[cfg(merman_internal_theme_acceptance)]
    title_fill_css: Option<Box<str>>,
    #[cfg(merman_internal_theme_acceptance)]
    title_bounds: Option<Bounds>,
    #[cfg(merman_internal_theme_acceptance)]
    title_fragment_digest: Option<[u8; 32]>,
    #[cfg(merman_internal_theme_acceptance)]
    title_run_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureGroupTextStyle {
    #[cfg(merman_internal_theme_acceptance)]
    id: Box<str>,
    title_style: Option<Box<str>>,
    title_fill: Option<FamilyThemeMechanismKey>,
    #[cfg(merman_internal_theme_acceptance)]
    title_fill_css: Option<Box<str>>,
    #[cfg(merman_internal_theme_acceptance)]
    title_bounds: Option<Bounds>,
    #[cfg(merman_internal_theme_acceptance)]
    title_fragment_digest: Option<[u8; 32]>,
    #[cfg(merman_internal_theme_acceptance)]
    title_run_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureEdgeSurfaceStyle {
    #[cfg(merman_internal_theme_acceptance)]
    lhs_id: Box<str>,
    #[cfg(merman_internal_theme_acceptance)]
    rhs_id: Box<str>,
    label_style: Option<Box<str>>,
    label_fill: Option<FamilyThemeMechanismKey>,
    #[cfg(merman_internal_theme_acceptance)]
    label_fill_css: Option<Box<str>>,
    lhs_arrow_style: Option<Box<str>>,
    lhs_arrow_fill: Option<FamilyThemeMechanismKey>,
    rhs_arrow_style: Option<Box<str>>,
    rhs_arrow_fill: Option<FamilyThemeMechanismKey>,
    #[cfg(merman_internal_theme_acceptance)]
    label_bounds: Option<Bounds>,
    #[cfg(merman_internal_theme_acceptance)]
    label_fragment_digest: Option<[u8; 32]>,
    #[cfg(merman_internal_theme_acceptance)]
    label_run_count: usize,
}

#[derive(Debug, Default)]
struct ArchitectureRuleOutcome {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

#[derive(Debug, Clone)]
enum ArchitectureMechanismOutcome {
    AppliedCandidate(BTreeSet<ThemeCapability>),
    NotApplicable,
    Residual(FamilyThemeResidualReason),
    Incomplete,
}

/// Resolved output styles and the rules observed by successful surface writers.
#[derive(Debug)]
pub(crate) struct ArchitectureSurfaceRenderState {
    services: Vec<ArchitectureServiceSurfaceStyle>,
    groups: Vec<ArchitectureGroupTextStyle>,
    edges: Vec<ArchitectureEdgeSurfaceStyle>,
    mechanism_outcomes: BTreeMap<FamilyThemeMechanismKey, ArchitectureMechanismOutcome>,
    emitted_rules: BTreeSet<FamilyThemeMechanismKey>,
}

impl ArchitectureSurfaceRenderState {
    pub(super) fn build(
        theme: &ResolvedDiagramTheme,
        ownership: ArchitectureSurfaceOwnership,
        services: impl IntoIterator<Item = ArchitectureServiceTerminal>,
        groups: impl IntoIterator<Item = ArchitectureGroupTerminal>,
        edges: impl IntoIterator<Item = ArchitectureEdgeTerminal>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut mechanisms = ArchitectureTerminalMechanisms::default();
        let mut service_styles = Vec::new();
        let mut text_ordinal = 0usize;
        for service in services {
            debug_assert_eq!(service.index, service_styles.len());
            let ordinal = service.index.saturating_add(1);
            let (background_fill, background_stroke) = if service.has_background {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                mechanisms.observe_terminal_style(
                    theme,
                    ThemeTarget::Node,
                    ordinal,
                    &style,
                    &[
                        (ResolvedStyleProperty::Fill, false),
                        (ResolvedStyleProperty::Stroke, ownership.service_stroke),
                    ],
                );
                (
                    resolve_direct_static_fill(
                        theme,
                        &style,
                        &[ThemeTarget::Node],
                        DirectStaticSelectorDomain::Unqualified,
                    ),
                    (!ownership.service_stroke)
                        .then(|| {
                            resolve_direct_static_stroke(
                                theme,
                                &style,
                                &[ThemeTarget::Node],
                                DirectStaticSelectorDomain::Unqualified,
                            )
                        })
                        .flatten(),
                )
            } else {
                (None, None)
            };
            let background_style =
                compose_paint_style(background_fill.as_ref(), background_stroke.as_ref());

            let title_fill = resolve_text_fill(
                service.has_title,
                &mut ArchitectureTextResolution {
                    theme,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    mechanisms: &mut mechanisms,
                },
            )?;
            let title_style = compose_paint_style(title_fill.as_ref(), None);

            service_styles.push(ArchitectureServiceSurfaceStyle {
                #[cfg(merman_internal_theme_acceptance)]
                id: Box::default(),
                background_style,
                background_fill: background_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Node)),
                background_stroke: background_stroke
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Node)),
                title_style,
                title_fill: title_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Text)),
                #[cfg(merman_internal_theme_acceptance)]
                title_fill_css: title_fill.map(|paint| paint.into_parts().0),
                #[cfg(merman_internal_theme_acceptance)]
                title_bounds: None,
                #[cfg(merman_internal_theme_acceptance)]
                title_fragment_digest: None,
                #[cfg(merman_internal_theme_acceptance)]
                title_run_count: 0,
            });
        }

        let mut group_styles = Vec::new();
        for group in groups {
            debug_assert_eq!(group.index, group_styles.len());
            let title_fill = resolve_text_fill(
                group.has_title,
                &mut ArchitectureTextResolution {
                    theme,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    mechanisms: &mut mechanisms,
                },
            )?;
            let title_style = compose_paint_style(title_fill.as_ref(), None);
            group_styles.push(ArchitectureGroupTextStyle {
                #[cfg(merman_internal_theme_acceptance)]
                id: Box::default(),
                title_style,
                title_fill: title_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Text)),
                #[cfg(merman_internal_theme_acceptance)]
                title_fill_css: title_fill.map(|paint| paint.into_parts().0),
                #[cfg(merman_internal_theme_acceptance)]
                title_bounds: None,
                #[cfg(merman_internal_theme_acceptance)]
                title_fragment_digest: None,
                #[cfg(merman_internal_theme_acceptance)]
                title_run_count: 0,
            });
        }

        let mut marker_ordinal = 0usize;
        let mut edge_styles = Vec::new();
        for edge in edges {
            debug_assert_eq!(edge.index, edge_styles.len());
            let label_fill = resolve_text_fill(
                edge.has_label,
                &mut ArchitectureTextResolution {
                    theme,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    mechanisms: &mut mechanisms,
                },
            )?;
            let label_style = compose_paint_style(label_fill.as_ref(), None);

            let mut arrow_fill =
                |present: bool| -> Result<Option<DirectStaticPaint>, OperationWorkError> {
                    if !present {
                        return Ok(None);
                    }
                    marker_ordinal = marker_ordinal.saturating_add(1);
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Marker,
                        ThemeVariant::Default,
                        Some(marker_ordinal),
                        work_meter,
                    )?;
                    mechanisms.observe_terminal_style(
                        theme,
                        ThemeTarget::Marker,
                        marker_ordinal,
                        &style,
                        &[(ResolvedStyleProperty::Fill, ownership.arrow_fill)],
                    );
                    Ok((!ownership.arrow_fill)
                        .then(|| {
                            resolve_direct_static_fill(
                                theme,
                                &style,
                                &[ThemeTarget::Marker],
                                DirectStaticSelectorDomain::Unqualified,
                            )
                        })
                        .flatten())
                };
            let lhs_arrow_fill = arrow_fill(edge.has_lhs_arrow)?;
            let rhs_arrow_fill = arrow_fill(edge.has_rhs_arrow)?;
            let lhs_arrow_style = compose_paint_style(lhs_arrow_fill.as_ref(), None);
            let rhs_arrow_style = compose_paint_style(rhs_arrow_fill.as_ref(), None);

            edge_styles.push(ArchitectureEdgeSurfaceStyle {
                #[cfg(merman_internal_theme_acceptance)]
                lhs_id: Box::default(),
                #[cfg(merman_internal_theme_acceptance)]
                rhs_id: Box::default(),
                label_style,
                label_fill: label_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Text)),
                #[cfg(merman_internal_theme_acceptance)]
                label_fill_css: label_fill.map(|paint| paint.into_parts().0),
                lhs_arrow_style,
                lhs_arrow_fill: lhs_arrow_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Marker)),
                rhs_arrow_style,
                rhs_arrow_fill: rhs_arrow_fill
                    .as_ref()
                    .map(|paint| paint_key(paint, ThemeTarget::Marker)),
                #[cfg(merman_internal_theme_acceptance)]
                label_bounds: None,
                #[cfg(merman_internal_theme_acceptance)]
                label_fragment_digest: None,
                #[cfg(merman_internal_theme_acceptance)]
                label_run_count: 0,
            });
        }

        let mechanism_outcomes = resolve_mechanism_outcomes(theme, &mechanisms);

        Ok(Self {
            services: service_styles,
            groups: group_styles,
            edges: edge_styles,
            mechanism_outcomes,
            emitted_rules: BTreeSet::new(),
        })
    }

    pub(crate) fn service_background_style(&self, service_index: usize) -> Option<&str> {
        self.services
            .get(service_index)
            .and_then(|style| style.background_style.as_deref())
    }

    pub(crate) fn service_title_style(&self, service_index: usize) -> Option<&str> {
        self.services
            .get(service_index)
            .and_then(|style| style.title_style.as_deref())
    }

    pub(crate) fn group_title_style(&self, group_index: usize) -> Option<&str> {
        self.groups
            .get(group_index)
            .and_then(|style| style.title_style.as_deref())
    }

    pub(crate) fn edge_label_style(&self, edge_index: usize) -> Option<&str> {
        self.edges
            .get(edge_index)
            .and_then(|style| style.label_style.as_deref())
    }

    pub(crate) fn edge_arrow_style(
        &self,
        edge_index: usize,
        side: ArchitectureArrowSide,
    ) -> Option<&str> {
        let style = self.edges.get(edge_index)?;
        match side {
            ArchitectureArrowSide::Lhs => style.lhs_arrow_style.as_deref(),
            ArchitectureArrowSide::Rhs => style.rhs_arrow_style.as_deref(),
        }
    }

    pub(crate) fn record_checkpointed_service(
        &mut self,
        emission: ArchitectureServiceTerminalEmission,
    ) {
        let style = &mut self.services[emission.index];
        if emission.background.emitted {
            record_paint(style.background_fill.as_ref(), &mut self.emitted_rules);
            record_paint(style.background_stroke.as_ref(), &mut self.emitted_rules);
        }
        if emission.title.emitted {
            record_paint(style.title_fill.as_ref(), &mut self.emitted_rules);
        }
        #[cfg(merman_internal_theme_acceptance)]
        {
            style.id = emission.id.into();
            style.title_bounds = emission.title.bounds;
            style.title_fragment_digest = emission.title.fragment_digest;
            style.title_run_count = emission.title.run_count;
        }
    }

    pub(crate) fn record_checkpointed_group(
        &mut self,
        emission: ArchitectureGroupTerminalEmission,
    ) {
        let style = &mut self.groups[emission.index];
        if emission.title.emitted {
            record_paint(style.title_fill.as_ref(), &mut self.emitted_rules);
        }
        #[cfg(merman_internal_theme_acceptance)]
        {
            style.id = emission.id.into();
            style.title_bounds = emission.title.bounds;
            style.title_fragment_digest = emission.title.fragment_digest;
            style.title_run_count = emission.title.run_count;
        }
    }

    pub(crate) fn record_checkpointed_edge(&mut self, emission: ArchitectureEdgeTerminalEmission) {
        let style = &mut self.edges[emission.index];
        if emission.label.emitted {
            record_paint(style.label_fill.as_ref(), &mut self.emitted_rules);
        }
        if emission.lhs_arrow.emitted {
            record_paint(style.lhs_arrow_fill.as_ref(), &mut self.emitted_rules);
        }
        if emission.rhs_arrow.emitted {
            record_paint(style.rhs_arrow_fill.as_ref(), &mut self.emitted_rules);
        }
        #[cfg(merman_internal_theme_acceptance)]
        {
            style.lhs_id = emission.lhs_id.into();
            style.rhs_id = emission.rhs_id.into();
            style.label_bounds = emission.label.bounds;
            style.label_fragment_digest = emission.label.fragment_digest;
            style.label_run_count = emission.label.run_count;
        }
    }

    pub(super) fn apply_to_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        for (key, outcome) in &self.mechanism_outcomes {
            match outcome {
                ArchitectureMechanismOutcome::AppliedCandidate(capabilities)
                    if self.emitted_rules.contains(key) =>
                {
                    evidence
                        .mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
                }
                ArchitectureMechanismOutcome::AppliedCandidate(_) => {}
                ArchitectureMechanismOutcome::NotApplicable => {
                    evidence.mark_not_applicable(key.clone());
                }
                ArchitectureMechanismOutcome::Residual(reason) => {
                    evidence.mark_residual(key.clone(), *reason);
                }
                ArchitectureMechanismOutcome::Incomplete => {}
            }
        }
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) fn architecture_text_receipt(
        &self,
    ) -> Option<crate::__private::ArchitectureTextReceipt> {
        let mut terminals = Vec::new();
        for service in &self.services {
            append_text_receipt_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextRole::Service,
                service.id.as_ref(),
                service.title_fill_css.as_deref(),
                service.title_bounds.as_ref(),
                service.title_fragment_digest,
                service.title_run_count,
            )?;
        }
        for group in &self.groups {
            append_text_receipt_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextRole::GroupTitle,
                group.id.as_ref(),
                group.title_fill_css.as_deref(),
                group.title_bounds.as_ref(),
                group.title_fragment_digest,
                group.title_run_count,
            )?;
        }
        for (index, edge) in self.edges.iter().enumerate() {
            let identity = format!("{}->{}#{index}", edge.lhs_id, edge.rhs_id);
            append_text_receipt_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextRole::EdgeLabel,
                &identity,
                edge.label_fill_css.as_deref(),
                edge.label_bounds.as_ref(),
                edge.label_fragment_digest,
                edge.label_run_count,
            )?;
        }
        crate::__private::ArchitectureTextReceipt::seal(terminals)
    }
}

#[cfg(merman_internal_theme_acceptance)]
fn append_text_receipt_terminal(
    terminals: &mut Vec<crate::__private::ArchitectureTextTerminal>,
    role: crate::__private::ArchitectureTextRole,
    identity: &str,
    fill_css: Option<&str>,
    bounds: Option<&Bounds>,
    fragment_digest: Option<[u8; 32]>,
    run_count: usize,
) -> Option<()> {
    let fill_css = fill_css?;
    let bounds = bounds?;
    let fragment_digest = fragment_digest?;
    terminals.push(
        crate::__private::ArchitectureTextTerminal::with_writer_facts(
            role,
            identity,
            fill_css,
            [
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            ],
            fragment_digest,
            run_count,
        )?,
    );
    Some(())
}

impl ArchitectureTerminalMechanisms {
    fn observe_terminal_style(
        &mut self,
        theme: &ResolvedDiagramTheme,
        target: ThemeTarget,
        ordinal: usize,
        style: &crate::diagram_theme::ResolvedThemeStyle,
        source_owned: &[(ResolvedStyleProperty, bool)],
    ) {
        self.winner_properties.extend(
            style
                .winner_rule_properties()
                .filter(|(property, _)| {
                    !source_owned
                        .iter()
                        .any(|(candidate, owned)| candidate == property && *owned)
                })
                .map(|(property, origin)| {
                    (
                        FamilyThemeMechanismKey::Rule {
                            index: origin.rule_index(),
                            target: origin.target(),
                        },
                        property,
                    )
                }),
        );
        if !source_owned
            .iter()
            .any(|(property, owned)| *property == ResolvedStyleProperty::Fill && *owned)
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            && theme.series_color(target, ordinal).is_some()
        {
            self.active_palettes
                .insert(FamilyThemeMechanismKey::OrdinalPalette { target });
        }
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
            theme.resolve_effect(target, style.effect_resolution())
        {
            self.active_effects
                .insert(FamilyThemeMechanismKey::EffectBinding {
                    target: binding.target(),
                    effect_id: binding.effect_id().to_owned(),
                });
        }
    }
}

fn resolve_mechanism_outcomes(
    theme: &ResolvedDiagramTheme,
    mechanisms: &ArchitectureTerminalMechanisms,
) -> BTreeMap<FamilyThemeMechanismKey, ArchitectureMechanismOutcome> {
    let mut rules = BTreeMap::<FamilyThemeMechanismKey, ArchitectureRuleOutcome>::new();
    let mut other = BTreeMap::new();
    for route in theme.family_mechanism_routes().iter().copied() {
        let Some(target) = mechanism_target(route.mechanism()) else {
            continue;
        };
        if !is_architecture_terminal_surface_target(target) {
            continue;
        }
        let key = theme.family_mechanism_key(route);
        match route.mechanism() {
            FamilyThemeMechanism::RuleFacet {
                selector, facet, ..
            } => {
                let property = resolved_style_property_for_facet(facet);
                let observation = rules.entry(key.clone()).or_default();
                if !mechanisms
                    .winner_properties
                    .contains(&(key.clone(), property))
                {
                    continue;
                }
                observation.applicable = true;
                match route.disposition() {
                    FamilyThemeDisposition::Unsupported => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                    FamilyThemeDisposition::TypedAdapter
                        if direct_surface_facet(target, selector, facet) =>
                    {
                        add_capabilities(&mut observation.capabilities, facet);
                    }
                    FamilyThemeDisposition::TypedAdapter => observation.incomplete = true,
                }
            }
            FamilyThemeMechanism::OrdinalPalette { .. } => {
                other.insert(
                    key.clone(),
                    if mechanisms.active_palettes.contains(&key) {
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                ArchitectureMechanismOutcome::Residual(
                                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                                )
                            }
                            FamilyThemeDisposition::TypedAdapter => {
                                ArchitectureMechanismOutcome::Incomplete
                            }
                        }
                    } else {
                        ArchitectureMechanismOutcome::NotApplicable
                    },
                );
            }
            FamilyThemeMechanism::EffectBinding { .. } => {
                other.insert(
                    key.clone(),
                    if mechanisms.active_effects.contains(&key) {
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                ArchitectureMechanismOutcome::Residual(
                                    FamilyThemeResidualReason::UnsupportedEffect,
                                )
                            }
                            FamilyThemeDisposition::TypedAdapter => {
                                ArchitectureMechanismOutcome::Incomplete
                            }
                        }
                    } else {
                        ArchitectureMechanismOutcome::NotApplicable
                    },
                );
            }
            FamilyThemeMechanism::BaseTypography(_) => {}
        }
    }

    other.extend(rules.into_iter().map(|(key, observation)| {
        let outcome = if !observation.applicable {
            ArchitectureMechanismOutcome::NotApplicable
        } else if let Some(reason) = observation.residual {
            ArchitectureMechanismOutcome::Residual(reason)
        } else if observation.incomplete || observation.capabilities.is_empty() {
            ArchitectureMechanismOutcome::Incomplete
        } else {
            ArchitectureMechanismOutcome::AppliedCandidate(observation.capabilities)
        };
        (key, outcome)
    }));
    other
}

fn resolve_text_fill(
    present: bool,
    resolution: &mut ArchitectureTextResolution<'_>,
) -> Result<Option<DirectStaticPaint>, OperationWorkError> {
    if !present {
        return Ok(None);
    }
    *resolution.ordinal = resolution.ordinal.saturating_add(1);
    let style = resolution.theme.style_with_work_meter(
        ThemeTarget::Text,
        ThemeVariant::Default,
        Some(*resolution.ordinal),
        resolution.work_meter,
    )?;
    resolution.mechanisms.observe_terminal_style(
        resolution.theme,
        ThemeTarget::Text,
        *resolution.ordinal,
        &style,
        &[(ResolvedStyleProperty::Fill, resolution.source_owned)],
    );
    Ok((!resolution.source_owned)
        .then(|| {
            resolve_direct_static_fill(
                resolution.theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        })
        .flatten())
}

fn paint_key(paint: &DirectStaticPaint, target: ThemeTarget) -> FamilyThemeMechanismKey {
    FamilyThemeMechanismKey::Rule {
        index: paint.rule_index(),
        target,
    }
}

fn record_paint(
    key: Option<&FamilyThemeMechanismKey>,
    emitted: &mut BTreeSet<FamilyThemeMechanismKey>,
) {
    if let Some(key) = key {
        emitted.insert(key.clone());
    }
}

fn compose_paint_style(
    fill: Option<&DirectStaticPaint>,
    stroke: Option<&DirectStaticPaint>,
) -> Option<Box<str>> {
    let mut style = String::new();
    if let Some(fill) = fill {
        style.push_str("fill:");
        style.push_str(fill.css());
        style.push(';');
    }
    if let Some(stroke) = stroke {
        style.push_str("stroke:");
        style.push_str(stroke.css());
        style.push(';');
    }
    (!style.is_empty()).then(|| style.into_boxed_str())
}

fn direct_surface_facet(
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> bool {
    let selector_is_direct = matches!(
        (target, selector),
        (
            ThemeTarget::Text,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            },
        ) | (_, FamilyThemeSelectorShape::Static { variant: None })
    );
    if !selector_is_direct {
        return false;
    }
    matches!(
        (target, facet),
        (
            ThemeTarget::Node,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ),
        ) | (
            ThemeTarget::Text | ThemeTarget::Marker,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ),
        )
    )
}

fn add_capabilities(capabilities: &mut BTreeSet<ThemeCapability>, facet: FamilyThemeRuleFacet) {
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

pub(super) fn mechanism_target(mechanism: FamilyThemeMechanism) -> Option<ThemeTarget> {
    match mechanism {
        FamilyThemeMechanism::BaseTypography(_) => None,
        FamilyThemeMechanism::RuleFacet { target, .. }
        | FamilyThemeMechanism::OrdinalPalette { target }
        | FamilyThemeMechanism::EffectBinding { target, .. } => Some(target),
    }
}

pub(super) fn is_architecture_terminal_surface_target(target: ThemeTarget) -> bool {
    matches!(
        target,
        ThemeTarget::Node | ThemeTarget::Text | ThemeTarget::Marker
    )
}
