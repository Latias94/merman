use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::config::config_css_number_or_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackPlan, StaticSurfacePaint, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::EventModelingDiagramLayout;
use crate::resources::OperationWorkMeter;

mod text_facts;
use text_facts::EventModelingTextFacts;

#[derive(Debug, Default)]
struct EventModelingTextRuleObservation {
    applicable: bool,
    pending_fill: Option<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

#[derive(Debug, Default)]
struct EventModelingCorePaints {
    node: StaticSurfacePaint,
    cluster: StaticSurfacePaint,
    edge: StaticSurfacePaint,
    marker: StaticSurfacePaint,
    node_label: StaticSurfacePaint,
    cluster_label: StaticSurfacePaint,
}

/// Final Event Modeling text styling shared by terminal emission and family evidence.
#[derive(Debug)]
pub(crate) struct EventModelingTextThemePlan {
    css_binding: super::EventModelingCssBinding,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Option<Box<str>>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    text_facts: EventModelingTextFacts,
    evidence: FamilyThemeOutcomes,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
    core_paints: EventModelingCorePaints,
    box_ownership: Vec<(bool, bool)>,
    terminal_emitted: OnceLock<()>,
}

impl EventModelingTextThemePlan {
    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        mut css_binding: super::EventModelingCssBinding,
        layout: &EventModelingDiagramLayout,
        prepared_labels: &super::EventModelingPreparedLabels,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let Some(theme) = theme else {
            let configured_font_size_css = config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .map(String::into_boxed_str);
            return Ok(Self::baseline(
                css_binding,
                inherited_font_stack,
                configured_font_size_css,
            ));
        };

        let typed_font_size = theme.family_mechanism_routes().iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        });
        let configured_font_size_css = config_css_number_or_string(
            effective_config.as_value(),
            &["themeVariables", "fontSize"],
        );
        let config_owns_font_size = typed_font_size
            && configured_font_size_css
                .as_deref()
                .and_then(crate::mermaid_style::css_font_size_value_override)
                .is_some()
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size && !config_owns_font_size;
        let font_size_css = if typed_font_size_active {
            Some(format!("{}px", theme.typography().font_size_px()).into_boxed_str())
        } else {
            configured_font_size_css.map(String::into_boxed_str)
        };
        let text_facts = EventModelingTextFacts::from_prepared(
            layout,
            prepared_labels,
            font_size_css.as_deref(),
            work_meter,
        )?;

        let config_owns_fill =
            effective_config.config_paint_overrides_typed_default("themeVariables.textColor");
        let owns = |path: &str| effective_config.config_paint_overrides_typed_default(path);
        let owns_role = |primary: &str, fallback: &str| {
            let key = if crate::config::config_string(
                effective_config.as_value(),
                &["themeVariables", primary],
            )
            .is_some()
            {
                primary
            } else {
                fallback
            };
            owns(&format!("themeVariables.{key}"))
        };
        let mut source_fill = Vec::new();
        let mut source_stroke = Vec::new();
        for box_layout in &layout.boxes {
            let role = match box_layout.model_entity_type.as_str() {
                "ui" => Some("Ui"),
                "pcr" | "processor" => Some("Processor"),
                "rmo" | "readmodel" => Some("ReadModel"),
                "cmd" | "command" => Some("Command"),
                "evt" | "event" => Some("Event"),
                _ => None,
            };
            source_fill
                .push(role.is_some_and(|role| owns(&format!("themeVariables.em{role}Fill"))));
            source_stroke
                .push(role.is_some_and(|role| owns(&format!("themeVariables.em{role}Stroke"))));
        }
        let node = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Node,
            &source_fill,
            &source_stroke,
            work_meter,
        )?;
        let box_ownership = source_fill
            .iter()
            .zip(&source_stroke)
            .map(|(&fill_owned, &stroke_owned)| (fill_owned, stroke_owned))
            .collect();
        let lane_count = layout.swimlanes.len();
        let cluster = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Cluster,
            &vec![owns_role("emSwimlaneBackgroundOdd", "emSwimlaneBackground"); lane_count],
            &vec![owns_role("emSwimlaneBackgroundStroke", "emSwimlaneBorder"); lane_count],
            work_meter,
        )?;
        let edge_count = layout.relations.len();
        let edge = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Edge,
            &vec![true; edge_count],
            &vec![owns("themeVariables.emRelationStroke"); edge_count],
            work_meter,
        )?;
        let marker = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::Marker,
            &vec![owns("themeVariables.emArrowhead"); usize::from(edge_count != 0)],
            &vec![false; usize::from(edge_count != 0)],
            work_meter,
        )?;
        let box_fill_owned = text_facts
            .boxes
            .iter()
            .map(|facts| config_owns_fill || text_facts::source_owns_fill(facts))
            .collect::<Vec<_>>();
        let box_hidden = text_facts
            .boxes
            .iter()
            .map(|facts| !text_facts::visible(facts))
            .collect::<Vec<_>>();
        let lane_fill_owned = text_facts
            .swimlanes
            .iter()
            .map(|facts| config_owns_fill || text_facts::source_owns_fill(facts))
            .collect::<Vec<_>>();
        let lane_hidden = text_facts
            .swimlanes
            .iter()
            .map(|facts| !text_facts::visible(facts))
            .collect::<Vec<_>>();
        let node_label = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::NodeLabel,
            &box_fill_owned,
            &box_hidden,
            work_meter,
        )?;
        let cluster_label = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::ClusterLabel,
            &lane_fill_owned,
            &lane_hidden,
            work_meter,
        )?;
        if let Some(fill) = &cluster.fill {
            css_binding.swimlane_background_fill = fill.css().to_owned();
        }
        if let Some(stroke) = &cluster.stroke {
            css_binding.swimlane_background_stroke = stroke.css().to_owned();
        }
        if let Some(stroke) = &edge.stroke {
            css_binding.relation_stroke = stroke.css().to_owned();
        }
        if let Some(fill) = &marker.fill {
            css_binding.arrowhead_fill = fill.css().to_owned();
        }
        let style = text_facts
            .all()
            .next()
            .is_some()
            .then(|| {
                theme.text_style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )
            })
            .transpose()?;
        // Text ordinals follow DOM emission, including terminals hidden by their final size.
        // Role paint overrides only fill, leaving unsupported sibling properties observable.
        let source_owned_fill = lane_fill_owned
            .iter()
            .map(|owned| *owned || cluster_label.fill.is_some())
            .chain(
                box_fill_owned
                    .iter()
                    .map(|owned| *owned || node_label.fill.is_some()),
            )
            .collect::<Vec<_>>();
        let mut winner_properties = BTreeSet::new();
        let mut unverified_fill_winners = BTreeSet::new();
        for (index, (fill_owned, facts)) in source_owned_fill
            .iter()
            .copied()
            .zip(text_facts.all())
            .enumerate()
        {
            if !text_facts::visible(facts) {
                continue;
            }
            let occurrence_style = theme.style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                Some(index + 1),
                work_meter,
            )?;
            winner_properties.extend(
                occurrence_style
                    .winner_rule_properties()
                    .filter(|(property, _)| *property != ResolvedStyleProperty::Fill || !fill_owned)
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
            if !fill_owned && text_facts::unverified_fill(facts) {
                unverified_fill_winners.extend(
                    occurrence_style
                        .winner_rule_properties()
                        .filter(|(property, _)| *property == ResolvedStyleProperty::Fill)
                        .map(|(_, origin)| origin.rule_index()),
                );
            }
        }
        let role_fill_covers_text = source_owned_fill.iter().all(|owned| *owned);
        let typed_fill = (!config_owns_fill && !role_fill_covers_text)
            .then(|| {
                style.as_ref().and_then(|style| {
                    resolve_direct_static_fill(
                        theme,
                        style,
                        &[ThemeTarget::Text],
                        DirectStaticSelectorDomain::Unqualified,
                    )
                })
            })
            .flatten();

        let mut evidence = FamilyThemeOutcomes::default();
        let mut observations = BTreeMap::<usize, EventModelingTextRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    selector: _,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }
                    observation.applicable = true;

                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Fill(_))
                            if typed_fill
                                .as_ref()
                                .is_some_and(|fill| fill.rule_index() == rule_index) =>
                        {
                            observation.pending_fill =
                                typed_fill.as_ref().map(DirectStaticPaint::capability);
                            if unverified_fill_winners.contains(&rule_index) {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedPaint);
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => {
                    // Reconciled below from the final text style of each actual terminal.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    // Reconciled below from the final text style of each actual terminal.
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Title,
                    ..
                }
                | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                }
                | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Event Modeling has no visual title terminal. Accessibility metadata is root
                    // chrome and is intentionally not the semantic Title styling target.
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter => {}
                // Base typography is reconciled by property in `record_evidence`. The shared
                // inherited-font plan records every Unsupported property so mixed requests do
                // not make a supported sibling disappear from the terminal writer.
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let visible_text = EventModelingTextFacts::visible_ordinals(text_facts.all());
        let visible_boxes = EventModelingTextFacts::visible_ordinals(text_facts.boxes.iter());
        let visible_lanes = EventModelingTextFacts::visible_ordinals(text_facts.swimlanes.iter());
        for route in theme.family_mechanism_routes().iter().copied() {
            let absent = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. } => match target {
                    ThemeTarget::Text => visible_text.is_empty(),
                    ThemeTarget::NodeLabel => visible_boxes.is_empty(),
                    ThemeTarget::ClusterLabel => visible_lanes.is_empty(),
                    _ => false,
                },
                _ => false,
            };
            if absent {
                evidence.mark_not_applicable(theme.family_mechanism_key(route));
            }
        }
        for (target, paint, facts) in [
            (ThemeTarget::NodeLabel, &node_label, &text_facts.boxes),
            (
                ThemeTarget::ClusterLabel,
                &cluster_label,
                &text_facts.swimlanes,
            ),
        ] {
            if let Some(fill) = paint.fill.as_ref()
                && facts
                    .iter()
                    .any(|facts| text_facts::visible(facts) && text_facts::unverified_fill(facts))
            {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Rule {
                        index: fill.rule_index(),
                        target,
                    },
                    FamilyThemeResidualReason::UnsupportedPaint,
                );
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Text,
                    TerminalVariantDomain::indexed(&visible_text),
                )
                .with_source_owned_fill(&source_owned_fill),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::NodeLabel,
                    TerminalVariantDomain::indexed(&visible_boxes),
                )
                .with_source_owned_fill(&box_fill_owned)
                .with_source_owned_stroke(&box_hidden),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::ClusterLabel,
                    TerminalVariantDomain::indexed(&visible_lanes),
                )
                .with_source_owned_fill(&lane_fill_owned)
                .with_source_owned_stroke(&lane_hidden),
            ],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Text,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if let Some(capability) = observation.pending_fill {
                pending.insert(key, capability);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        if let Some(fill) = typed_fill {
            css_binding.text_color = fill.css().to_owned();
        }
        Ok(Self {
            css_binding,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: typed_font_size,
            typed_font_size_active,
            text_facts,
            evidence,
            pending,
            core_paints: EventModelingCorePaints {
                node,
                cluster,
                edge,
                marker,
                node_label,
                cluster_label,
            },
            box_ownership,
            terminal_emitted: OnceLock::new(),
        })
    }

    fn baseline(
        css_binding: super::EventModelingCssBinding,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Option<Box<str>>,
    ) -> Self {
        Self {
            css_binding,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: false,
            typed_font_size_active: false,
            text_facts: EventModelingTextFacts::default(),
            evidence: FamilyThemeOutcomes::default(),
            pending: BTreeMap::new(),
            core_paints: EventModelingCorePaints::default(),
            box_ownership: Vec::new(),
            terminal_emitted: OnceLock::new(),
        }
    }

    pub(crate) fn css_binding(&self) -> &super::EventModelingCssBinding {
        &self.css_binding
    }

    pub(crate) fn box_fill<'a>(&'a self, ordinal: usize, baseline: &'a str) -> &'a str {
        self.box_ownership
            .get(ordinal)
            .filter(|(fill_owned, _)| !*fill_owned)
            .and(self.core_paints.node.fill.as_ref())
            .map(DirectStaticPaint::css)
            .unwrap_or(baseline)
    }

    pub(crate) fn box_stroke<'a>(&'a self, ordinal: usize, baseline: &'a str) -> &'a str {
        self.box_ownership
            .get(ordinal)
            .filter(|(_, stroke_owned)| !*stroke_owned)
            .and(self.core_paints.node.stroke.as_ref())
            .map(DirectStaticPaint::css)
            .unwrap_or(baseline)
    }

    pub(crate) fn relation_stroke<'a>(&'a self, baseline: &'a str) -> &'a str {
        self.core_paints
            .edge
            .stroke
            .as_ref()
            .map_or(baseline, DirectStaticPaint::css)
    }

    pub(crate) fn record_terminal(&self) {
        let _ = self.terminal_emitted.set(());
    }

    pub(crate) fn write_stylesheet(
        &self,
        out: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
    ) {
        let fill = &self.css_binding.text_color;
        let font = self.inherited_font_stack.font_family_css();
        let _ = write!(
            out,
            "#{diagram_id} {{ fill: {fill}; color: {fill}; }}#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {font};"
        );
        if let Some(font_size) = self.font_size_css.as_deref() {
            let _ = write!(out, " font-size: {font_size};");
        }
        let _ = write!(out, " }}#{diagram_id} .em-relation {{ fill: none; }}");
        for (paint, selector) in [
            (&self.core_paints.node_label, ".em-box span"),
            (&self.core_paints.cluster_label, ".em-swimlane text"),
        ] {
            if let Some(fill) = paint.fill.as_ref() {
                let _ = write!(
                    out,
                    "#{diagram_id} {selector} {{ fill: {}; color: {}; }}",
                    fill.css(),
                    fill.css()
                );
            }
        }
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        if self.terminal_emitted.get().is_some() {
            for paint in [
                &self.core_paints.node,
                &self.core_paints.cluster,
                &self.core_paints.edge,
                &self.core_paints.marker,
                &self.core_paints.node_label,
                &self.core_paints.cluster_label,
            ] {
                paint.record_evidence(evidence);
            }
        }
        for (key, capability) in &self.pending {
            if self.terminal_emitted.get().is_some() {
                evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        let has_visible_terminals = self.text_facts.all().any(text_facts::visible);
        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, has_visible_terminals);
        let properties = [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
            ),
        ];
        for (property, requested, active) in properties {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            let (inherited, unknown) = self.text_facts.typography(property);
            if !has_visible_terminals || !active {
                evidence.mark_not_applicable(key);
            } else if unknown || self.terminal_emitted.get().is_none() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            } else if !inherited {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
        }
    }
}
