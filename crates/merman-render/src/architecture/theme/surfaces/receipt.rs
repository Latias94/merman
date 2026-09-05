use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, ResolvedThemeEffect, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::Bounds;
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureServiceTerminal<'a> {
    pub(crate) index: usize,
    pub(crate) id: &'a str,
    pub(crate) has_background: bool,
    pub(crate) has_title: bool,
    pub(crate) has_icon_text: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureGroupTerminal<'a> {
    pub(crate) index: usize,
    pub(crate) id: &'a str,
    pub(crate) has_title: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureEdgeTerminal<'a> {
    pub(crate) index: usize,
    pub(crate) lhs_id: &'a str,
    pub(crate) rhs_id: &'a str,
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
pub(crate) struct ArchitecturePaintTerminalEmission<'a> {
    pub(crate) emitted: bool,
    pub(crate) style: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureTextTerminalEmission<'a> {
    pub(crate) emitted: bool,
    pub(crate) style: Option<&'a str>,
    pub(crate) bounds: Option<&'a Bounds>,
    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fragment_digest: Option<[u8; 32]>,
    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) run_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureServiceTerminalEmission<'a> {
    pub(crate) index: usize,
    pub(crate) id: &'a str,
    pub(crate) background: ArchitecturePaintTerminalEmission<'a>,
    pub(crate) title: ArchitectureTextTerminalEmission<'a>,
    pub(crate) has_icon_text: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureGroupTerminalEmission<'a> {
    pub(crate) index: usize,
    pub(crate) id: &'a str,
    pub(crate) title: ArchitectureTextTerminalEmission<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ArchitectureEdgeTerminalEmission<'a> {
    pub(crate) index: usize,
    pub(crate) lhs_id: &'a str,
    pub(crate) rhs_id: &'a str,
    pub(crate) label: ArchitectureTextTerminalEmission<'a>,
    pub(crate) lhs_arrow: ArchitecturePaintTerminalEmission<'a>,
    pub(crate) rhs_arrow: ArchitecturePaintTerminalEmission<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArchitectureSurfaceOwnership {
    pub(super) service_stroke: bool,
    pub(super) text_fill: bool,
    pub(super) arrow_fill: bool,
}

struct ArchitectureTextResolution<'a> {
    theme: &'a ResolvedDiagramTheme,
    direct_rule_facets: &'a BTreeSet<(usize, ThemeTarget, FamilyThemeRuleFacet)>,
    source_owned: bool,
    ordinal: &'a mut usize,
    work_meter: &'a OperationWorkMeter,
    winner_properties: &'a mut BTreeSet<(FamilyThemeMechanismKey, ResolvedStyleProperty)>,
    active_palettes: &'a mut BTreeSet<FamilyThemeMechanismKey>,
    active_effects: &'a mut BTreeSet<FamilyThemeMechanismKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchitectureExpectedPaint {
    rule_index: usize,
    target: ThemeTarget,
    css: Box<str>,
}

#[derive(Debug, Clone)]
struct ArchitectureServiceSurfaceExpectation {
    id: Box<str>,
    has_background: bool,
    background_style: Option<Box<str>>,
    background_fill: Option<ArchitectureExpectedPaint>,
    background_stroke: Option<ArchitectureExpectedPaint>,
    has_title: bool,
    title_style: Option<Box<str>>,
    title_fill: Option<ArchitectureExpectedPaint>,
    has_icon_text: bool,
    checkpointed: bool,
    #[cfg(feature = "internal-theme-acceptance")]
    title_bounds: Option<Bounds>,
    #[cfg(feature = "internal-theme-acceptance")]
    title_fragment_digest: Option<[u8; 32]>,
    #[cfg(feature = "internal-theme-acceptance")]
    title_run_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureGroupTextExpectation {
    id: Box<str>,
    has_title: bool,
    title_style: Option<Box<str>>,
    title_fill: Option<ArchitectureExpectedPaint>,
    checkpointed: bool,
    #[cfg(feature = "internal-theme-acceptance")]
    title_bounds: Option<Bounds>,
    #[cfg(feature = "internal-theme-acceptance")]
    title_fragment_digest: Option<[u8; 32]>,
    #[cfg(feature = "internal-theme-acceptance")]
    title_run_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureEdgeSurfaceExpectation {
    lhs_id: Box<str>,
    rhs_id: Box<str>,
    has_label: bool,
    label_style: Option<Box<str>>,
    label_fill: Option<ArchitectureExpectedPaint>,
    has_lhs_arrow: bool,
    lhs_arrow_style: Option<Box<str>>,
    lhs_arrow_fill: Option<ArchitectureExpectedPaint>,
    has_rhs_arrow: bool,
    rhs_arrow_style: Option<Box<str>>,
    rhs_arrow_fill: Option<ArchitectureExpectedPaint>,
    checkpointed: bool,
    #[cfg(feature = "internal-theme-acceptance")]
    label_bounds: Option<Bounds>,
    #[cfg(feature = "internal-theme-acceptance")]
    label_fragment_digest: Option<[u8; 32]>,
    #[cfg(feature = "internal-theme-acceptance")]
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

/// Writer-owned exact receipt for every family-local Architecture surface occurrence.
#[derive(Debug)]
pub(crate) struct ArchitectureSurfaceThemeReceipt {
    services: Vec<ArchitectureServiceSurfaceExpectation>,
    groups: Vec<ArchitectureGroupTextExpectation>,
    edges: Vec<ArchitectureEdgeSurfaceExpectation>,
    mechanism_outcomes: BTreeMap<FamilyThemeMechanismKey, ArchitectureMechanismOutcome>,
    expected_by_rule: BTreeMap<FamilyThemeMechanismKey, usize>,
    emitted_by_rule: BTreeMap<FamilyThemeMechanismKey, usize>,
    terminals_match: bool,
}

impl ArchitectureSurfaceThemeReceipt {
    pub(super) fn build<'a>(
        theme: &ResolvedDiagramTheme,
        ownership: ArchitectureSurfaceOwnership,
        services: impl IntoIterator<Item = ArchitectureServiceTerminal<'a>>,
        groups: impl IntoIterator<Item = ArchitectureGroupTerminal<'a>>,
        edges: impl IntoIterator<Item = ArchitectureEdgeTerminal<'a>>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut winner_properties = BTreeSet::new();
        let mut active_palettes = BTreeSet::new();
        let mut active_effects = BTreeSet::new();
        let mut expected_by_rule = BTreeMap::new();
        let direct_rule_facets = theme
            .family_mechanism_routes()
            .iter()
            .filter_map(|route| match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } if route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && direct_surface_facet(target, selector, facet) =>
                {
                    Some((rule_index, target, facet))
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => None,
            })
            .collect::<BTreeSet<_>>();

        let mut service_expectations = Vec::new();
        let mut text_ordinal = 0usize;
        for service in services {
            debug_assert_eq!(service.index, service_expectations.len());
            let ordinal = service.index.saturating_add(1);
            let (background_fill, background_stroke) = if service.has_background {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                observe_terminal_style(
                    theme,
                    ThemeTarget::Node,
                    ordinal,
                    &style,
                    [
                        (ResolvedStyleProperty::Fill, false),
                        (ResolvedStyleProperty::Stroke, ownership.service_stroke),
                    ],
                    &mut winner_properties,
                    &mut active_palettes,
                    &mut active_effects,
                );
                (
                    typed_terminal_paint(&direct_rule_facets, ThemeTarget::Node, &style, false),
                    (!ownership.service_stroke)
                        .then(|| {
                            typed_terminal_paint(
                                &direct_rule_facets,
                                ThemeTarget::Node,
                                &style,
                                true,
                            )
                        })
                        .flatten(),
                )
            } else {
                (None, None)
            };
            let background_style =
                compose_paint_style(background_fill.as_ref(), background_stroke.as_ref());
            count_expected(&mut expected_by_rule, background_fill.as_ref());
            count_expected(&mut expected_by_rule, background_stroke.as_ref());

            let title_fill = resolve_text_fill(
                service.has_title,
                &mut ArchitectureTextResolution {
                    theme,
                    direct_rule_facets: &direct_rule_facets,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    winner_properties: &mut winner_properties,
                    active_palettes: &mut active_palettes,
                    active_effects: &mut active_effects,
                },
            )?;
            let title_style = compose_paint_style(title_fill.as_ref(), None);
            count_expected(&mut expected_by_rule, title_fill.as_ref());

            service_expectations.push(ArchitectureServiceSurfaceExpectation {
                id: service.id.into(),
                has_background: service.has_background,
                background_style,
                background_fill,
                background_stroke,
                has_title: service.has_title,
                title_style,
                title_fill,
                has_icon_text: service.has_icon_text,
                checkpointed: false,
                #[cfg(feature = "internal-theme-acceptance")]
                title_bounds: None,
                #[cfg(feature = "internal-theme-acceptance")]
                title_fragment_digest: None,
                #[cfg(feature = "internal-theme-acceptance")]
                title_run_count: 0,
            });
        }

        let mut group_expectations = Vec::new();
        for group in groups {
            debug_assert_eq!(group.index, group_expectations.len());
            let title_fill = resolve_text_fill(
                group.has_title,
                &mut ArchitectureTextResolution {
                    theme,
                    direct_rule_facets: &direct_rule_facets,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    winner_properties: &mut winner_properties,
                    active_palettes: &mut active_palettes,
                    active_effects: &mut active_effects,
                },
            )?;
            let title_style = compose_paint_style(title_fill.as_ref(), None);
            count_expected(&mut expected_by_rule, title_fill.as_ref());
            group_expectations.push(ArchitectureGroupTextExpectation {
                id: group.id.into(),
                has_title: group.has_title,
                title_style,
                title_fill,
                checkpointed: false,
                #[cfg(feature = "internal-theme-acceptance")]
                title_bounds: None,
                #[cfg(feature = "internal-theme-acceptance")]
                title_fragment_digest: None,
                #[cfg(feature = "internal-theme-acceptance")]
                title_run_count: 0,
            });
        }

        let mut marker_ordinal = 0usize;
        let mut edge_expectations = Vec::new();
        for edge in edges {
            debug_assert_eq!(edge.index, edge_expectations.len());
            let label_fill = resolve_text_fill(
                edge.has_label,
                &mut ArchitectureTextResolution {
                    theme,
                    direct_rule_facets: &direct_rule_facets,
                    source_owned: ownership.text_fill,
                    ordinal: &mut text_ordinal,
                    work_meter,
                    winner_properties: &mut winner_properties,
                    active_palettes: &mut active_palettes,
                    active_effects: &mut active_effects,
                },
            )?;
            let label_style = compose_paint_style(label_fill.as_ref(), None);
            count_expected(&mut expected_by_rule, label_fill.as_ref());

            let mut arrow_fill =
                |present: bool| -> Result<Option<ArchitectureExpectedPaint>, OperationWorkError> {
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
                    observe_terminal_style(
                        theme,
                        ThemeTarget::Marker,
                        marker_ordinal,
                        &style,
                        [(ResolvedStyleProperty::Fill, ownership.arrow_fill)],
                        &mut winner_properties,
                        &mut active_palettes,
                        &mut active_effects,
                    );
                    Ok((!ownership.arrow_fill)
                        .then(|| {
                            typed_terminal_paint(
                                &direct_rule_facets,
                                ThemeTarget::Marker,
                                &style,
                                false,
                            )
                        })
                        .flatten())
                };
            let lhs_arrow_fill = arrow_fill(edge.has_lhs_arrow)?;
            let rhs_arrow_fill = arrow_fill(edge.has_rhs_arrow)?;
            let lhs_arrow_style = compose_paint_style(lhs_arrow_fill.as_ref(), None);
            let rhs_arrow_style = compose_paint_style(rhs_arrow_fill.as_ref(), None);
            count_expected(&mut expected_by_rule, lhs_arrow_fill.as_ref());
            count_expected(&mut expected_by_rule, rhs_arrow_fill.as_ref());

            edge_expectations.push(ArchitectureEdgeSurfaceExpectation {
                lhs_id: edge.lhs_id.into(),
                rhs_id: edge.rhs_id.into(),
                has_label: edge.has_label,
                label_style,
                label_fill,
                has_lhs_arrow: edge.has_lhs_arrow,
                lhs_arrow_style,
                lhs_arrow_fill,
                has_rhs_arrow: edge.has_rhs_arrow,
                rhs_arrow_style,
                rhs_arrow_fill,
                checkpointed: false,
                #[cfg(feature = "internal-theme-acceptance")]
                label_bounds: None,
                #[cfg(feature = "internal-theme-acceptance")]
                label_fragment_digest: None,
                #[cfg(feature = "internal-theme-acceptance")]
                label_run_count: 0,
            });
        }

        let mechanism_outcomes = resolve_mechanism_outcomes(
            theme,
            &winner_properties,
            &active_palettes,
            &active_effects,
            &expected_by_rule,
        );

        Ok(Self {
            services: service_expectations,
            groups: group_expectations,
            edges: edge_expectations,
            mechanism_outcomes,
            expected_by_rule,
            emitted_by_rule: BTreeMap::new(),
            terminals_match: true,
        })
    }

    pub(crate) fn service_background_style(&self, service_index: usize, id: &str) -> Option<&str> {
        self.services
            .get(service_index)
            .filter(|expectation| expectation.id.as_ref() == id)
            .and_then(|expectation| expectation.background_style.as_deref())
    }

    pub(crate) fn service_title_style(&self, service_index: usize, id: &str) -> Option<&str> {
        self.services
            .get(service_index)
            .filter(|expectation| expectation.id.as_ref() == id)
            .and_then(|expectation| expectation.title_style.as_deref())
    }

    pub(crate) fn group_title_style(&self, group_index: usize, id: &str) -> Option<&str> {
        self.groups
            .get(group_index)
            .filter(|expectation| expectation.id.as_ref() == id)
            .and_then(|expectation| expectation.title_style.as_deref())
    }

    pub(crate) fn edge_label_style(
        &self,
        edge_index: usize,
        lhs_id: &str,
        rhs_id: &str,
    ) -> Option<&str> {
        self.edge(edge_index, lhs_id, rhs_id)
            .and_then(|expectation| expectation.label_style.as_deref())
    }

    pub(crate) fn edge_arrow_style(
        &self,
        edge_index: usize,
        lhs_id: &str,
        rhs_id: &str,
        side: ArchitectureArrowSide,
    ) -> Option<&str> {
        let expectation = self.edge(edge_index, lhs_id, rhs_id)?;
        match side {
            ArchitectureArrowSide::Lhs => expectation.lhs_arrow_style.as_deref(),
            ArchitectureArrowSide::Rhs => expectation.rhs_arrow_style.as_deref(),
        }
    }

    pub(crate) fn record_checkpointed_service(
        &mut self,
        emission: ArchitectureServiceTerminalEmission<'_>,
    ) {
        let Some(expectation) = self.services.get_mut(emission.index) else {
            self.terminals_match = false;
            return;
        };
        self.terminals_match &= !expectation.checkpointed && expectation.id.as_ref() == emission.id;
        expectation.checkpointed = true;
        self.terminals_match &= expectation.has_background == emission.background.emitted;
        self.terminals_match &=
            expectation.background_style.as_deref() == emission.background.style;
        self.terminals_match &= expectation.has_title == emission.title.emitted;
        self.terminals_match &= expectation.title_style.as_deref() == emission.title.style;
        record_text_bounds(
            expectation.has_title,
            emission.title.bounds,
            &mut self.terminals_match,
        );
        #[cfg(feature = "internal-theme-acceptance")]
        {
            expectation.title_bounds = emission.title.bounds.cloned();
            expectation.title_fragment_digest = emission.title.fragment_digest;
            expectation.title_run_count = emission.title.run_count;
            self.terminals_match &= emission.title.emitted
                == emission.title.fragment_digest.is_some()
                && (!emission.title.emitted || emission.title.run_count != 0);
        }
        self.terminals_match &= expectation.has_icon_text == emission.has_icon_text;
        record_paint(
            expectation.background_fill.as_ref(),
            emission.background.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
        record_paint(
            expectation.background_stroke.as_ref(),
            emission.background.style,
            "stroke",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
        record_paint(
            expectation.title_fill.as_ref(),
            emission.title.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
    }

    pub(crate) fn record_checkpointed_group(
        &mut self,
        emission: ArchitectureGroupTerminalEmission<'_>,
    ) {
        let Some(expectation) = self.groups.get_mut(emission.index) else {
            self.terminals_match = false;
            return;
        };
        self.terminals_match &= !expectation.checkpointed && expectation.id.as_ref() == emission.id;
        expectation.checkpointed = true;
        self.terminals_match &= expectation.has_title == emission.title.emitted;
        self.terminals_match &= expectation.title_style.as_deref() == emission.title.style;
        record_text_bounds(
            expectation.has_title,
            emission.title.bounds,
            &mut self.terminals_match,
        );
        #[cfg(feature = "internal-theme-acceptance")]
        {
            expectation.title_bounds = emission.title.bounds.cloned();
            expectation.title_fragment_digest = emission.title.fragment_digest;
            expectation.title_run_count = emission.title.run_count;
            self.terminals_match &= emission.title.emitted
                == emission.title.fragment_digest.is_some()
                && (!emission.title.emitted || emission.title.run_count != 0);
        }
        record_paint(
            expectation.title_fill.as_ref(),
            emission.title.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
    }

    pub(crate) fn record_checkpointed_edge(
        &mut self,
        emission: ArchitectureEdgeTerminalEmission<'_>,
    ) {
        let Some(expectation) = self.edges.get_mut(emission.index) else {
            self.terminals_match = false;
            return;
        };
        self.terminals_match &= !expectation.checkpointed
            && expectation.lhs_id.as_ref() == emission.lhs_id
            && expectation.rhs_id.as_ref() == emission.rhs_id;
        expectation.checkpointed = true;
        self.terminals_match &= expectation.has_label == emission.label.emitted;
        self.terminals_match &= expectation.label_style.as_deref() == emission.label.style;
        record_text_bounds(
            expectation.has_label,
            emission.label.bounds,
            &mut self.terminals_match,
        );
        #[cfg(feature = "internal-theme-acceptance")]
        {
            expectation.label_bounds = emission.label.bounds.cloned();
            expectation.label_fragment_digest = emission.label.fragment_digest;
            expectation.label_run_count = emission.label.run_count;
            self.terminals_match &= emission.label.emitted
                == emission.label.fragment_digest.is_some()
                && (!emission.label.emitted || emission.label.run_count != 0);
        }
        self.terminals_match &= expectation.has_lhs_arrow == emission.lhs_arrow.emitted;
        self.terminals_match &= expectation.lhs_arrow_style.as_deref() == emission.lhs_arrow.style;
        self.terminals_match &= expectation.has_rhs_arrow == emission.rhs_arrow.emitted;
        self.terminals_match &= expectation.rhs_arrow_style.as_deref() == emission.rhs_arrow.style;
        record_paint(
            expectation.label_fill.as_ref(),
            emission.label.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
        record_paint(
            expectation.lhs_arrow_fill.as_ref(),
            emission.lhs_arrow.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
        record_paint(
            expectation.rhs_arrow_fill.as_ref(),
            emission.rhs_arrow.style,
            "fill",
            &mut self.emitted_by_rule,
            &mut self.terminals_match,
        );
    }

    fn edge(
        &self,
        edge_index: usize,
        lhs_id: &str,
        rhs_id: &str,
    ) -> Option<&ArchitectureEdgeSurfaceExpectation> {
        self.edges.get(edge_index).filter(|expectation| {
            expectation.lhs_id.as_ref() == lhs_id && expectation.rhs_id.as_ref() == rhs_id
        })
    }

    pub(super) fn proves_complete(&self) -> bool {
        self.terminals_match
            && self.services.iter().all(|entry| entry.checkpointed)
            && self.groups.iter().all(|entry| entry.checkpointed)
            && self.edges.iter().all(|entry| entry.checkpointed)
    }

    fn proves_rule(&self, key: &FamilyThemeMechanismKey) -> bool {
        self.expected_by_rule.get(key).copied().unwrap_or(0) != 0
            && self.expected_by_rule.get(key) == self.emitted_by_rule.get(key)
    }

    pub(super) fn apply_to_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        for (key, outcome) in &self.mechanism_outcomes {
            match outcome {
                ArchitectureMechanismOutcome::AppliedCandidate(capabilities)
                    if self.proves_rule(key) =>
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

    #[cfg(feature = "internal-theme-acceptance")]
    pub(super) fn architecture_text_cutover_receipt(
        &self,
    ) -> Option<crate::__private::ArchitectureTextCutoverReceipt> {
        if !self.proves_complete() {
            return None;
        }
        let mut terminals = Vec::new();
        for service in &self.services {
            append_text_cutover_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextCutoverRole::Service,
                service.id.as_ref(),
                service.title_fill.as_ref(),
                service.title_bounds.as_ref(),
                service.title_fragment_digest,
                service.title_run_count,
            )?;
        }
        for group in &self.groups {
            append_text_cutover_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextCutoverRole::GroupTitle,
                group.id.as_ref(),
                group.title_fill.as_ref(),
                group.title_bounds.as_ref(),
                group.title_fragment_digest,
                group.title_run_count,
            )?;
        }
        for (index, edge) in self.edges.iter().enumerate() {
            let identity = format!("{}->{}#{index}", edge.lhs_id, edge.rhs_id);
            append_text_cutover_terminal(
                &mut terminals,
                crate::__private::ArchitectureTextCutoverRole::EdgeLabel,
                &identity,
                edge.label_fill.as_ref(),
                edge.label_bounds.as_ref(),
                edge.label_fragment_digest,
                edge.label_run_count,
            )?;
        }
        crate::__private::ArchitectureTextCutoverReceipt::seal(terminals)
    }
}

#[cfg(feature = "internal-theme-acceptance")]
fn append_text_cutover_terminal(
    terminals: &mut Vec<crate::__private::ArchitectureTextCutoverTerminal>,
    role: crate::__private::ArchitectureTextCutoverRole,
    identity: &str,
    paint: Option<&ArchitectureExpectedPaint>,
    bounds: Option<&Bounds>,
    fragment_digest: Option<[u8; 32]>,
    run_count: usize,
) -> Option<()> {
    let paint = paint?;
    if paint.target != ThemeTarget::Text {
        return None;
    }
    let bounds = bounds?;
    let fragment_digest = fragment_digest?;
    terminals.push(
        crate::__private::ArchitectureTextCutoverTerminal::with_writer_facts(
            role,
            identity,
            paint.css.as_ref(),
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

fn record_text_bounds(expected: bool, emitted: Option<&Bounds>, terminals_match: &mut bool) {
    *terminals_match &= expected == emitted.is_some();
    let Some(bounds) = emitted else {
        return;
    };
    *terminals_match &= [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        && bounds.max_x >= bounds.min_x
        && bounds.max_y >= bounds.min_y;
}

fn observe_terminal_style(
    theme: &ResolvedDiagramTheme,
    target: ThemeTarget,
    ordinal: usize,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    properties: impl IntoIterator<Item = (ResolvedStyleProperty, bool)>,
    winner_properties: &mut BTreeSet<(FamilyThemeMechanismKey, ResolvedStyleProperty)>,
    active_palettes: &mut BTreeSet<FamilyThemeMechanismKey>,
    active_effects: &mut BTreeSet<FamilyThemeMechanismKey>,
) {
    let source_owned = properties.into_iter().collect::<Vec<_>>();
    winner_properties.extend(style.winner_rule_properties().into_iter().filter_map(
        |(property, origin)| {
            (!source_owned
                .iter()
                .any(|(candidate, owned)| *candidate == property && *owned))
            .then(|| {
                (
                    FamilyThemeMechanismKey::Rule {
                        index: origin.rule_index(),
                        target: origin.target(),
                    },
                    property,
                )
            })
        },
    ));
    if !source_owned
        .iter()
        .any(|(property, owned)| *property == ResolvedStyleProperty::Fill && *owned)
        && matches!(style.fill_resolution().specified(), Specified::Unspecified)
        && theme.series_color(target, ordinal).is_some()
    {
        active_palettes.insert(FamilyThemeMechanismKey::OrdinalPalette { target });
    }
    if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
        theme.resolve_effect(target, style.effect_resolution())
    {
        active_effects.insert(FamilyThemeMechanismKey::EffectBinding {
            target: binding.target(),
            effect_id: binding.effect_id().to_owned(),
        });
    }
}

fn resolve_mechanism_outcomes(
    theme: &ResolvedDiagramTheme,
    winner_properties: &BTreeSet<(FamilyThemeMechanismKey, ResolvedStyleProperty)>,
    active_palettes: &BTreeSet<FamilyThemeMechanismKey>,
    active_effects: &BTreeSet<FamilyThemeMechanismKey>,
    expected_by_rule: &BTreeMap<FamilyThemeMechanismKey, usize>,
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
                if !winner_properties.contains(&(key.clone(), property)) {
                    continue;
                }
                observation.applicable = true;
                match route.disposition() {
                    FamilyThemeDisposition::Unsupported => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                    FamilyThemeDisposition::LegacyCompatibility => {
                        observation.incomplete = true;
                    }
                    FamilyThemeDisposition::TypedAdapter
                        if direct_surface_facet(target, selector, facet)
                            && expected_by_rule.get(&key).copied().unwrap_or(0) != 0 =>
                    {
                        add_capabilities(&mut observation.capabilities, facet);
                    }
                    FamilyThemeDisposition::TypedAdapter => observation.incomplete = true,
                }
            }
            FamilyThemeMechanism::OrdinalPalette { .. } => {
                other.insert(
                    key.clone(),
                    if active_palettes.contains(&key) {
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                ArchitectureMechanismOutcome::Residual(
                                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                                )
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::LegacyCompatibility => {
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
                    if active_effects.contains(&key) {
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                ArchitectureMechanismOutcome::Residual(
                                    FamilyThemeResidualReason::UnsupportedEffect,
                                )
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::LegacyCompatibility => {
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
) -> Result<Option<ArchitectureExpectedPaint>, OperationWorkError> {
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
    observe_terminal_style(
        resolution.theme,
        ThemeTarget::Text,
        *resolution.ordinal,
        &style,
        [(ResolvedStyleProperty::Fill, resolution.source_owned)],
        resolution.winner_properties,
        resolution.active_palettes,
        resolution.active_effects,
    );
    Ok((!resolution.source_owned)
        .then(|| {
            typed_terminal_paint(
                resolution.direct_rule_facets,
                ThemeTarget::Text,
                &style,
                false,
            )
        })
        .flatten())
}

fn typed_terminal_paint(
    direct_rule_facets: &BTreeSet<(usize, ThemeTarget, FamilyThemeRuleFacet)>,
    target: ThemeTarget,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    stroke: bool,
) -> Option<ArchitectureExpectedPaint> {
    let resolution = if stroke {
        style.stroke_resolution()
    } else {
        style.fill_resolution()
    };
    let origin = resolution.winner()?;
    let facet = if stroke {
        FamilyThemeRuleFacet::stroke(resolution.specified())?
    } else {
        FamilyThemeRuleFacet::fill(resolution.specified())?
    };
    if origin.target() != target
        || !direct_rule_facets.contains(&(origin.rule_index(), target, facet))
    {
        return None;
    }
    let css = match resolution.specified() {
        Specified::Value(CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(CanvasPaint::Solid(color)) => color.as_css().into_boxed_str(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(ArchitectureExpectedPaint {
        rule_index: origin.rule_index(),
        target,
        css,
    })
}

fn count_expected(
    counts: &mut BTreeMap<FamilyThemeMechanismKey, usize>,
    paint: Option<&ArchitectureExpectedPaint>,
) {
    if let Some(paint) = paint {
        *counts
            .entry(FamilyThemeMechanismKey::Rule {
                index: paint.rule_index,
                target: paint.target,
            })
            .or_default() += 1;
    }
}

fn record_paint(
    expected: Option<&ArchitectureExpectedPaint>,
    style: Option<&str>,
    property: &str,
    emitted: &mut BTreeMap<FamilyThemeMechanismKey, usize>,
    terminals_match: &mut bool,
) {
    let Some(expected) = expected else {
        return;
    };
    let matches = terminal_paint(style, property) == Some(expected.css.as_ref());
    *terminals_match &= matches;
    if matches {
        *emitted
            .entry(FamilyThemeMechanismKey::Rule {
                index: expected.rule_index,
                target: expected.target,
            })
            .or_default() += 1;
    }
}

fn compose_paint_style(
    fill: Option<&ArchitectureExpectedPaint>,
    stroke: Option<&ArchitectureExpectedPaint>,
) -> Option<Box<str>> {
    let mut style = String::new();
    if let Some(fill) = fill {
        style.push_str("fill:");
        style.push_str(&fill.css);
        style.push(';');
    }
    if let Some(stroke) = stroke {
        style.push_str("stroke:");
        style.push_str(&stroke.css);
        style.push(';');
    }
    (!style.is_empty()).then(|| style.into_boxed_str())
}

fn terminal_paint<'a>(style: Option<&'a str>, property: &str) -> Option<&'a str> {
    style?
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .filter(|declaration| declaration.property() == property)
        .map(|declaration| declaration.value())
        .next_back()
}

fn direct_surface_facet(
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> bool {
    let selector_is_direct = match (target, selector) {
        (
            ThemeTarget::Text,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            },
        )
        | (_, FamilyThemeSelectorShape::Static { variant: None }) => true,
        _ => false,
    };
    if !selector_is_direct {
        return false;
    }
    match (target, facet) {
        (
            ThemeTarget::Node,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            )
            | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ),
        )
        | (
            ThemeTarget::Text | ThemeTarget::Marker,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ),
        ) => true,
        _ => false,
    }
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
