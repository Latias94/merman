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
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EventModelingTextOccurrences {
    swimlanes: usize,
    boxes: usize,
}

impl EventModelingTextOccurrences {
    fn from_layout(layout: &EventModelingDiagramLayout) -> Self {
        Self {
            swimlanes: layout
                .swimlanes
                .iter()
                .filter(|swimlane| !swimlane.label.trim().is_empty())
                .count(),
            boxes: layout
                .boxes
                .iter()
                .filter(|box_layout| !box_layout.text.trim().is_empty())
                .count(),
        }
    }

    const fn total(self) -> usize {
        self.swimlanes.saturating_add(self.boxes)
    }
}

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
    occurrences: EventModelingTextOccurrences,
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
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let occurrences = theme
            .map(|_| EventModelingTextOccurrences::from_layout(layout))
            .unwrap_or_default();
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let Some(theme) = theme else {
            let configured_font_size_css = config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .map(String::into_boxed_str);
            return Ok(Self::baseline_from_occurrences(
                css_binding,
                occurrences,
                inherited_font_stack,
                configured_font_size_css,
                false,
                false,
            ));
        };

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
        let node_label = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::NodeLabel,
            &vec![config_owns_fill; occurrences.boxes],
            &vec![false; occurrences.boxes],
            work_meter,
        )?;
        let cluster_label = StaticSurfacePaint::resolve(
            Some(theme),
            ThemeTarget::ClusterLabel,
            &vec![config_owns_fill; occurrences.swimlanes],
            &vec![false; occurrences.swimlanes],
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
        let style = (occurrences.total() != 0)
            .then(|| {
                theme.text_style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )
            })
            .transpose()?;
        // Text ordinals follow actual SVG emission: visible swimlane labels, then box labels.
        // Role paint overrides only fill, leaving unsupported sibling properties observable.
        let source_owned_fill = std::iter::repeat_n(
            config_owns_fill || cluster_label.fill.is_some(),
            occurrences.swimlanes,
        )
        .chain(std::iter::repeat_n(
            config_owns_fill || node_label.fill.is_some(),
            occurrences.boxes,
        ))
        .collect::<Vec<_>>();
        let mut winner_properties = BTreeSet::new();
        for (index, fill_owned) in source_owned_fill.iter().copied().enumerate() {
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
        }
        let role_fill_covers_text = (occurrences.boxes == 0 || node_label.fill.is_some())
            && (occurrences.swimlanes == 0 || cluster_label.fill.is_some());
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
        let mut typed_font_size = false;
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
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
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

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Text,
                TerminalVariantDomain::uniform(occurrences.total(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
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

        let config_owns_font_size = typed_font_size
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size && !config_owns_font_size;
        let font_size_css = if typed_font_size_active {
            Some(format!("{}px", theme.typography().font_size_px()).into_boxed_str())
        } else {
            config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .map(String::into_boxed_str)
        };

        if let Some(fill) = typed_fill {
            css_binding.text_color = fill.css().to_owned();
        }
        Ok(Self {
            css_binding,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: typed_font_size,
            typed_font_size_active,
            occurrences,
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

    fn baseline_from_occurrences(
        css_binding: super::EventModelingCssBinding,
        occurrences: EventModelingTextOccurrences,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Option<Box<str>>,
        typed_font_size_requested: bool,
        typed_font_size_active: bool,
    ) -> Self {
        Self {
            css_binding,
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            occurrences,
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
        evidence.merge_borrowed(&self.evidence);
        for (key, capability) in &self.pending {
            evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
        }
        let has_visible_terminals = self.occurrences.total() != 0;
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
            if !has_visible_terminals || !active {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
        }
    }
}
