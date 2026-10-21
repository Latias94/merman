use std::collections::{BTreeMap, BTreeSet};
#[path = "css_binding.rs"]
mod css_binding;
pub(crate) use css_binding::BlockCssThemeBinding;
use std::sync::OnceLock;

use indexmap::IndexMap;
use merman_core::diagrams::block::BlockClassDefRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Block edge-label background owner, shared with Flowchart.
#[derive(Debug)]
pub(crate) struct BlockLabelBackgroundPlan(crate::family::EdgeLabelBackgroundPlan, Box<str>);

impl BlockLabelBackgroundPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        configured: &str,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let inner = crate::family::EdgeLabelBackgroundPlan::resolve(theme, config, work)?;
        let color = Box::from(inner.color(configured));
        Ok(Self(inner, color))
    }

    pub(crate) fn requested(&self) -> bool {
        self.0.requested()
    }

    pub(crate) fn color(&self) -> &str {
        &self.1
    }

    pub(crate) fn record_stylesheet(&self) {
        self.0.record_stylesheet();
    }

    pub(crate) fn record_terminal(
        &self,
        has_area: bool,
        background_emitted: bool,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        self.0.record_terminal(has_area, background_emitted, work)
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        self.0.record_evidence(evidence);
    }
}

/// Per-node label paint, resolved against source ownership for the SVG writer.
#[derive(Debug)]
pub(crate) struct BlockNodeLabelPaintPlan {
    paints: Box<[Option<Box<str>>]>,
    evidence: FamilyThemeOutcomes,
}

impl BlockNodeLabelPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let html_labels = crate::config::config_effective_html_labels(config.as_value());
        let mut plan = Self {
            paints: Box::new([]),
            evidence: FamilyThemeOutcomes::default(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. }
                    if matches!(target, ThemeTarget::NodeLabel | ThemeTarget::Text)
            )
        }) {
            return Ok(plan);
        }
        let config_owned = [
            "themeVariables.nodeTextColor",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ]
        .into_iter()
        .any(|path| merman_core::__private::config_paint_overrides_typed_default(config, path));
        let has_ordinal = theme.family_rules().any(|(_, rule)| {
            matches!(rule.target(), ThemeTarget::NodeLabel | ThemeTarget::Text)
                && rule.ordinal().is_some()
        });
        let static_style = theme.text_style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            None,
            work,
        )?;
        let sources = super::resolve_block_node_sources(model);
        let color_classes = model
            .class_defs
            .values()
            .filter(|class| {
                class
                    .styles
                    .iter()
                    .any(|raw| owns_label_color(raw, "color"))
                    || (!html_labels
                        && class
                            .text_styles
                            .iter()
                            .any(|raw| owns_label_color(raw, "fill")))
            })
            .map(|class| class.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut source_owned = Vec::with_capacity(layout.nodes.len());
        let mut paints = Vec::with_capacity(layout.nodes.len());
        for (index, node) in layout.nodes.iter().enumerate() {
            work.charge(1)?;
            let source = sources.get(node.id.as_str());
            let owned = config_owned
                || source.is_some_and(|source| {
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "color"))
                        || if source.classes.is_empty() {
                            color_classes.contains("default")
                        } else {
                            source
                                .classes
                                .iter()
                                .any(|class| color_classes.contains(class.as_str()))
                        }
                });
            let label = source
                .map(|source| super::decode_block_label_html(source.label))
                .unwrap_or_default();
            let text = if html_labels {
                label
            } else {
                crate::graph_label::flowchart_label_plain_text_for_layout(&label, "text", false)
            };
            let visible = !text.trim().is_empty()
                && node.label_width.unwrap_or(0.0) > 0.0
                && node.label_height.unwrap_or(0.0) > 0.0;
            source_owned.push(owned || !visible);
            let dynamic_style;
            let style = if has_ordinal {
                dynamic_style = theme.text_style_with_work_meter(
                    ThemeTarget::NodeLabel,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work,
                )?;
                &dynamic_style
            } else {
                &static_style
            };
            if visible {
                winners.extend(
                    style
                        .winner_rule_properties()
                        .filter_map(|(property, origin)| {
                            (!(owned && property == ResolvedStyleProperty::Fill))
                                .then_some((origin.rule_index(), property))
                        }),
                );
            }
            let paint = if visible && !owned {
                resolve_direct_static_fill(
                    theme,
                    style,
                    &[ThemeTarget::Text, ThemeTarget::NodeLabel],
                    DirectStaticSelectorDomain::Default,
                )
                .map(DirectPaintExpectation::from_paint)
            } else {
                None
            };
            if let Some(paint) = paint.as_ref()
                && let Some((_, origin)) =
                    style.winner_rule_properties().find(|(property, origin)| {
                        *property == ResolvedStyleProperty::Fill
                            && origin.rule_index() == paint.rule_index()
                    })
            {
                capabilities.insert((paint.rule_index(), origin.target()), paint.capability());
            }
            paints.push(paint.map(|paint| paint.css().into()));
        }
        let mut observations = BTreeMap::<(usize, ThemeTarget), NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !matches!(target, ThemeTarget::NodeLabel | ThemeTarget::Text) {
                continue;
            }
            let observation = observations.entry((rule_index, target)).or_default();
            if !winners.contains(&(rule_index, resolved_style_property_for_facet(facet))) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        if let Some(capability) = capabilities.get(&(rule_index, target)) {
                            observation
                                .capabilities
                                .insert(ResolvedStyleProperty::Fill, *capability);
                        } else {
                            observation.incomplete = true;
                        }
                    } else {
                        observation.incomplete = true;
                    }
                }
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
            }
        }
        for ((index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule { index, target };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if !observation.incomplete
                && let Some(capability) = observation.capabilities.get(&ResolvedStyleProperty::Fill)
            {
                plan.evidence
                    .mark_applied_with_capabilities(key, [*capability]);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::NodeLabel,
                    TerminalVariantDomain::uniform(layout.nodes.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&source_owned),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Text,
                    TerminalVariantDomain::uniform(layout.nodes.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&source_owned),
            ],
            work,
        )?;
        plan.paints = paints.into();
        Ok(plan)
    }

    pub(crate) fn color(&self, node_index: usize) -> Option<&str> {
        self.paints.get(node_index)?.as_deref()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
    }
}

fn owns_label_color(raw: &str, property: &str) -> bool {
    crate::mermaid_style::parse_style_declaration(raw)
        .is_some_and(|declaration| declaration.property() == property)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockTypographyOutcome {
    Inactive,
    Typed,
    ConfigOwned,
}

/// Final Block typography shared by layout measurement and SVG CSS.
#[derive(Debug)]
pub(crate) struct BlockTypographyThemePlan {
    css_binding: BlockCssThemeBinding,
    padding: f64,
    text_style: crate::text::TextStyle,
    evidence: FamilyThemeOutcomes,
    outcome: BlockTypographyOutcome,
    unsupported_properties: BTreeSet<ThemeTypographyProperty>,
    typed_font_stack_requested: bool,
    typed_font_size_requested: bool,
    config_owns_font_stack: bool,
    config_owns_font_size: bool,
    visible_labels: OnceLock<bool>,
}

impl BlockTypographyThemePlan {
    pub(crate) fn prepare_class_definitions(
        &mut self,
        definitions: &indexmap::IndexMap<
            String,
            merman_core::diagrams::block::BlockClassDefRenderModel,
        >,
    ) {
        self.css_binding.prepare_class_definitions(definitions);
    }

    pub(crate) fn css_binding(&self) -> &BlockCssThemeBinding {
        &self.css_binding
    }
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
    ) -> crate::Result<Self> {
        let settings =
            super::config::BlockConfigView::new(effective_config.as_value()).layout_settings();
        let padding = settings.padding;
        let mut text_style = settings.text_style;
        let css_binding = BlockCssThemeBinding::resolve(effective_config.as_value())?;
        let Some(theme) = theme else {
            return Ok(Self {
                padding,
                text_style,
                css_binding,
                evidence: FamilyThemeOutcomes::default(),
                outcome: BlockTypographyOutcome::Inactive,
                unsupported_properties: BTreeSet::new(),
                typed_font_stack_requested: false,
                typed_font_size_requested: false,
                config_owns_font_stack: false,
                config_owns_font_size: false,
                visible_labels: OnceLock::new(),
            });
        };

        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_properties = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(property)
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_properties.insert(property);
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let config_owns_font_stack =
            ["themeVariables.fontFamily", "fontFamily"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let config_owns_font_size =
            ["themeVariables.fontSize", "fontSize"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let typed_font_stack_applied = typed_font_stack && !config_owns_font_stack;
        let typed_font_size_applied = typed_font_size && !config_owns_font_size;
        if typed_font_stack_applied {
            text_style.font_family = Some(theme.typography().font_stack().as_css());
        }
        if typed_font_size_applied {
            text_style.font_size = f64::from(theme.typography().font_size_px()).max(1.0);
        }

        let requested_typed = typed_font_stack || typed_font_size;
        let applied_typed = typed_font_stack_applied || typed_font_size_applied;
        let outcome = if applied_typed {
            BlockTypographyOutcome::Typed
        } else if requested_typed {
            BlockTypographyOutcome::ConfigOwned
        } else {
            BlockTypographyOutcome::Inactive
        };
        Ok(Self {
            padding,
            text_style,
            css_binding,
            evidence: FamilyThemeOutcomes::default(),
            outcome,
            unsupported_properties,
            typed_font_stack_requested: typed_font_stack,
            typed_font_size_requested: typed_font_size,
            config_owns_font_stack,
            config_owns_font_size,
            visible_labels: OnceLock::new(),
        })
    }

    pub(crate) const fn text_style(&self) -> &crate::text::TextStyle {
        &self.text_style
    }

    pub(crate) const fn padding(&self) -> f64 {
        self.padding
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.text_style.font_family.as_deref().unwrap_or_default()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.text_style.font_size
    }

    pub(crate) fn observes_visible_labels(&self) -> bool {
        self.typed_font_stack_requested
            || self.typed_font_size_requested
            || !self.unsupported_properties.is_empty()
    }

    pub(crate) fn record_visible_labels(&self, visible: bool) {
        let _ = self.visible_labels.set(visible);
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        let visible = self.visible_labels.get().copied();
        for property in &self.unsupported_properties {
            let key = FamilyThemeMechanismKey::Typography(*property);
            if visible == Some(false) {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        }
        for (property, requested, config_owned) in [
            (
                ThemeTypographyProperty::FontStack,
                self.typed_font_stack_requested,
                self.config_owns_font_stack,
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.config_owns_font_size,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            match self.outcome {
                BlockTypographyOutcome::ConfigOwned if config_owned => {
                    evidence.mark_not_applicable(key);
                }
                BlockTypographyOutcome::Typed if config_owned => {
                    evidence.mark_not_applicable(key);
                }
                BlockTypographyOutcome::Typed => match visible {
                    Some(true) => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                    }
                    Some(false) => {
                        evidence.mark_not_applicable(key);
                    }
                    None => {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    }
                },
                BlockTypographyOutcome::ConfigOwned | BlockTypographyOutcome::Inactive => {}
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BlockNodePaint {
    fill: Option<Box<str>>,
    stroke: Option<Box<str>>,
}

/// Block node shell paint resolved once for the SVG writer.
#[derive(Debug)]
pub(crate) struct BlockNodePaintThemePlan {
    bindings: Box<[Option<super::node_binding::BlockNodeTerminalBinding>]>,
    paints: Box<[BlockNodePaint]>,
    evidence: FamilyThemeOutcomes,
}

impl BlockNodePaintThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        labels: &BlockNodeLabelPaintPlan,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            let mut plan = Self::unthemed();
            plan.bindings = plan.bind_terminals(model, layout, labels, work_meter)?;
            return Ok(plan);
        };
        let mut paints = vec![BlockNodePaint::default(); layout.nodes.len()];
        let composites = layout
            .shape_geometries
            .iter()
            .filter_map(|geometry| {
                matches!(
                    geometry.boundary,
                    super::BlockShapeBoundary::Rectangle {
                        kind: super::BlockRectangleKind::Composite,
                        ..
                    }
                )
                .then_some(geometry.id.as_str())
            })
            .collect::<BTreeSet<_>>();
        let sources = super::resolve_block_node_sources(model);
        let ownership = BlockNodePaintSourceOwnership::new(&model.class_defs);
        let node_config_fill = mermaid_owns_node_fill(effective_config);
        let node_config_stroke = mermaid_owns_node_stroke(effective_config);
        let cluster_config_fill = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.clusterBkg",
        );
        let cluster_config_stroke = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.clusterBorder",
        );
        let has_ordinals = |target| {
            theme
                .family_rules()
                .any(|(_, rule)| rule.target() == target && rule.ordinal().is_some())
        };
        let node_ordinals = has_ordinals(ThemeTarget::Node);
        let cluster_ordinals = has_ordinals(ThemeTarget::Cluster);
        let node_style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let cluster_style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut node_owned_fill = Vec::with_capacity(paints.len());
        let mut node_owned_stroke = Vec::with_capacity(paints.len());
        let mut cluster_owned_fill = Vec::new();
        let mut cluster_owned_stroke = Vec::new();
        let mut cluster_index = 0;
        for (node_index, paint) in paints.iter_mut().enumerate() {
            work_meter.charge(1)?;
            let source = sources.get(layout.nodes[node_index].id.as_str());
            let source_fill = source.is_some_and(|source| {
                ownership.owns_fill(
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "fill")),
                    source.classes,
                )
            });
            let source_stroke = source.is_some_and(|source| {
                ownership.owns_stroke(
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "stroke")),
                    source.classes,
                )
            });
            let owned_fill = source_fill || node_config_fill;
            let owned_stroke = source_stroke || node_config_stroke;
            node_owned_fill.push(owned_fill);
            node_owned_stroke.push(owned_stroke);
            let ordinal_node_style;
            let style = if node_ordinals {
                ordinal_node_style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(node_index + 1),
                    work_meter,
                )?;
                &ordinal_node_style
            } else {
                &node_style
            };
            observe_shell_style(
                theme,
                style,
                ThemeTarget::Node,
                owned_fill,
                owned_stroke,
                paint,
                &mut winners,
                &mut capabilities,
            );
            if composites.contains(layout.nodes[node_index].id.as_str()) {
                cluster_index += 1;
                // Node paint that actually reached this shell owns its property. Node
                // capability gaps do not erase its residual or block Cluster's fallback.
                let owned_fill = source_fill || cluster_config_fill || paint.fill.is_some();
                let owned_stroke = source_stroke || cluster_config_stroke || paint.stroke.is_some();
                cluster_owned_fill.push(owned_fill);
                cluster_owned_stroke.push(owned_stroke);
                let ordinal_cluster_style;
                let style = if cluster_ordinals {
                    ordinal_cluster_style = theme.style_with_work_meter(
                        ThemeTarget::Cluster,
                        ThemeVariant::Default,
                        Some(cluster_index),
                        work_meter,
                    )?;
                    &ordinal_cluster_style
                } else {
                    &cluster_style
                };
                observe_shell_style(
                    theme,
                    style,
                    ThemeTarget::Cluster,
                    owned_fill,
                    owned_stroke,
                    paint,
                    &mut winners,
                    &mut capabilities,
                );
            }
        }
        let mut evidence = FamilyThemeOutcomes::default();
        let mut observations = BTreeMap::<FamilyThemeMechanismKey, NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !matches!(target, ThemeTarget::Node | ThemeTarget::Cluster) {
                continue;
            }
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            let observation = observations.entry(key).or_default();
            let property = resolved_style_property_for_facet(facet);
            if !winners.contains(&(rule_index, property)) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if let Some(capability) = capabilities.get(&(rule_index, property)) {
                        observation.capabilities.insert(property, *capability);
                    } else {
                        observation.incomplete = true;
                    }
                }
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(paints.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&node_owned_fill)
                .with_source_owned_stroke(&node_owned_stroke),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Cluster,
                    TerminalVariantDomain::uniform(cluster_index, ThemeVariant::Default),
                )
                .with_source_owned_fill(&cluster_owned_fill)
                .with_source_owned_stroke(&cluster_owned_stroke),
                // Composite labels remain node labels, and frontmatter has no title terminal.
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::ClusterLabel,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
            ],
            work_meter,
        )?;
        for (key, observation) in observations {
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if !observation.incomplete && !observation.capabilities.is_empty() {
                evidence
                    .mark_applied_with_capabilities(key, observation.capabilities.into_values());
            }
        }
        let mut plan = Self {
            bindings: Box::new([]),
            paints: paints.into(),
            evidence,
        };
        plan.bindings = plan.bind_terminals(model, layout, labels, work_meter)?;
        Ok(plan)
    }

    fn unthemed() -> Self {
        Self {
            bindings: Box::new([]),
            paints: Box::new([]),
            evidence: FamilyThemeOutcomes::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn baseline() -> Self {
        Self::unthemed()
    }

    fn bind_terminals(
        &self,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        labels: &BlockNodeLabelPaintPlan,
        work: &OperationWorkMeter,
    ) -> Result<Box<[Option<super::node_binding::BlockNodeTerminalBinding>]>, OperationWorkError>
    {
        work.charge(layout.nodes.len())?;
        let sources = super::resolve_block_node_sources(model);
        layout
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                work.charge(1)?;
                Ok(sources.get(node.id.as_str()).map(|source| {
                    super::node_binding::lower(
                        source.styles,
                        self.typed_fill(i),
                        self.typed_stroke(i),
                        labels.color(i),
                    )
                }))
            })
            .collect()
    }

    pub(crate) fn terminal_binding(
        &self,
        index: usize,
    ) -> Option<&super::node_binding::BlockNodeTerminalBinding> {
        self.bindings.get(index)?.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn has_terminal_bindings(&self) -> bool {
        !self.bindings.is_empty()
    }

    fn typed_stroke(&self, node_index: usize) -> Option<&str> {
        self.typed_paint(node_index, ResolvedStyleProperty::Stroke)
    }

    fn typed_fill(&self, node_index: usize) -> Option<&str> {
        self.typed_paint(node_index, ResolvedStyleProperty::Fill)
    }

    fn typed_paint(&self, node_index: usize, property: ResolvedStyleProperty) -> Option<&str> {
        self.paints.get(node_index)?.paint(property)
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
    }
}

impl BlockNodePaint {
    fn paint(&self, property: ResolvedStyleProperty) -> Option<&str> {
        match property {
            ResolvedStyleProperty::Fill => self.fill.as_deref(),
            ResolvedStyleProperty::Stroke => self.stroke.as_deref(),
            _ => None,
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Shell paint resolution reconciles theme rules and source ownership."
)]
fn observe_shell_style(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    target: ThemeTarget,
    owns_fill: bool,
    owns_stroke: bool,
    expectation: &mut BlockNodePaint,
    winners: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    capabilities: &mut BTreeMap<(usize, ResolvedStyleProperty), ThemeCapability>,
) {
    winners.extend(
        style
            .winner_rule_properties()
            .filter_map(|(property, origin)| {
                (!(owns_fill && property == ResolvedStyleProperty::Fill
                    || owns_stroke && property == ResolvedStyleProperty::Stroke))
                    .then_some((origin.rule_index(), property))
            }),
    );
    for (property, owned) in [
        (ResolvedStyleProperty::Fill, owns_fill),
        (ResolvedStyleProperty::Stroke, owns_stroke),
    ] {
        if owned {
            continue;
        }
        let paint = if property == ResolvedStyleProperty::Fill {
            resolve_direct_static_fill(theme, style, &[target], DirectStaticSelectorDomain::Default)
        } else {
            resolve_direct_static_stroke(
                theme,
                style,
                &[target],
                DirectStaticSelectorDomain::Default,
            )
        };
        if let Some(paint) = paint.map(DirectPaintExpectation::from_paint) {
            capabilities.insert((paint.rule_index(), property), paint.capability());
            if property == ResolvedStyleProperty::Fill {
                expectation.fill = Some(paint.css().into());
            } else {
                expectation.stroke = Some(paint.css().into());
            }
        }
    }
}

fn mermaid_owns_node_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_paint_overrides_typed_default(
        config,
        "themeVariables.nodeBorder",
    )
}

fn mermaid_owns_node_fill(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_paint_overrides_typed_default(config, "themeVariables.mainBkg")
}

#[derive(Debug, Default)]
struct NodeRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

/// Precomputed Mermaid source-style owners for Block node shell paint.
struct BlockNodePaintSourceOwnership {
    fill_classes: BTreeSet<String>,
    stroke_classes: BTreeSet<String>,
}

impl BlockNodePaintSourceOwnership {
    pub(crate) fn new(class_defs: &IndexMap<String, BlockClassDefRenderModel>) -> Self {
        let (fill_classes, stroke_classes) = classes_owning_node_paint(class_defs);
        Self {
            fill_classes,
            stroke_classes,
        }
    }

    pub(crate) fn owns_fill(&self, inline_owns_fill: bool, assigned_classes: &[String]) -> bool {
        source_owns_property(inline_owns_fill, assigned_classes, &self.fill_classes)
    }

    pub(crate) fn owns_stroke(
        &self,
        inline_owns_stroke: bool,
        assigned_classes: &[String],
    ) -> bool {
        source_owns_property(inline_owns_stroke, assigned_classes, &self.stroke_classes)
    }
}

fn classes_owning_node_paint(
    class_defs: &IndexMap<String, BlockClassDefRenderModel>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut fill_classes = BTreeSet::new();
    let mut stroke_classes = BTreeSet::new();
    for class_def in class_defs.values() {
        for declaration in &class_def.styles {
            let Some(parsed) = crate::mermaid_style::parse_style_declaration(declaration) else {
                continue;
            };
            match parsed.property() {
                "fill" => {
                    fill_classes.insert(class_def.id.clone());
                }
                "stroke" => {
                    stroke_classes.insert(class_def.id.clone());
                }
                "all" => {
                    fill_classes.insert(class_def.id.clone());
                    stroke_classes.insert(class_def.id.clone());
                }
                _ => {}
            }
        }
    }
    (fill_classes, stroke_classes)
}

fn source_owns_property(
    inline_owns_property: bool,
    assigned_classes: &[String],
    owning_classes: &BTreeSet<String>,
) -> bool {
    inline_owns_property
        || owning_classes.contains("node")
        || owning_classes.contains("flowchart-label")
        || (assigned_classes.is_empty() && owning_classes.contains("default"))
        || assigned_classes
            .iter()
            .any(|assigned| owning_classes.contains(assigned))
}

#[cfg(test)]
mod tests {
    fn collect_evidence(
        theme: &crate::diagram_theme::ResolvedDiagramTheme,
        record: impl FnOnce(&mut crate::family::FamilyThemeEvidence),
    ) -> crate::family::FamilyThemeEvidence {
        let mut evidence = crate::family::FamilyThemeEvidence::from_theme(Some(theme));
        record(&mut evidence);
        evidence
    }

    fn resolve_node_paint(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<BlockNodePaintThemePlan, OperationWorkError> {
        let labels = BlockNodeLabelPaintPlan::resolve(theme, config, model, layout, work)?;
        BlockNodePaintThemePlan::resolve(theme, config, model, layout, &labels, work)
    }

    use super::*;

    #[test]
    fn explicit_default_font_size_remains_a_typed_request() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    crate::DiagramFamilyId::BLOCK,
                    ThemeTextStyle::default().with_font_size_px(16.0).unwrap(),
                ),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let plan = BlockTypographyThemePlan::resolve(
            Some(&resolved),
            &merman_core::MermaidConfig::empty_object(),
        )
        .unwrap();
        assert!(plan.typed_font_size_requested);
        assert_eq!(plan.font_size_px(), 16.0);
    }

    #[test]
    fn unthemed_typography_keeps_config_style_without_terminal_evidence() {
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({
            "fontFamily": "Config Sans, Arial",
            "themeVariables": {"fontSize": "24px"},
            "block": {"padding": 12}
        }));
        let plan = BlockTypographyThemePlan::resolve(None, &config).unwrap();
        assert_eq!(plan.font_family_css(), "Config Sans,Arial");
        assert_eq!(plan.font_size_px(), 24.0);
        assert_eq!(plan.padding(), 12.0);
        assert!(!plan.observes_visible_labels());
        assert!(plan.visible_labels.get().is_none());
    }

    #[test]
    fn unsupported_typography_still_observes_visible_terminals() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    crate::DiagramFamilyId::BLOCK,
                    ThemeTextStyle::default().with_font_weight(700).unwrap(),
                ),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontWeight);
        for (label, not_applicable) in [(" \n", true), ("Visible", false)] {
            let plan = BlockTypographyThemePlan::resolve(
                Some(&resolved),
                &merman_core::MermaidConfig::empty_object(),
            )
            .unwrap();
            assert!(plan.observes_visible_labels());
            plan.record_visible_labels(!label.trim().is_empty());
            let evidence = collect_evidence(&resolved, |evidence| plan.record_evidence(evidence));
            assert_eq!(
                evidence.not_applicable_mechanisms().contains(&key),
                not_applicable
            );
            assert_eq!(evidence.residuals().is_empty(), not_applicable);
        }
    }

    #[test]
    fn paint_plan_preserves_nested_source_ownership_across_later_fragments() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
                ),
            ))
            .unwrap();
        let theme = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let config = merman_core::MermaidConfig::empty_object();
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };
        for class_owned in [false, true] {
            let source = if class_owned {
                "block\nA[\"Alpha\"]\nclassDef brand fill:#bb0000\nclass A brand\n"
            } else {
                "block\nA[\"Alpha\"]\nstyle A fill:#bb0000\n"
            };
            let parsed = merman_core::Engine::new()
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .expect("parse Block source ownership fixture")
                .expect("detect Block source ownership fixture");
            let mut model = match parsed.model() {
                merman_core::RenderSemanticModel::Block(model) => model.clone(),
                _ => panic!("expected Block render model"),
            };
            let layout = crate::block::layout_block_diagram_typed(
                &model,
                &serde_json::json!({}),
                &crate::text::DeterministicTextMeasurer::default(),
            )
            .unwrap();
            assert_eq!(layout.nodes.len(), 1);
            let plan =
                resolve_node_paint(Some(&theme), &config, &model, &layout, &work_meter).unwrap();
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert!(evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(evidence.residuals().is_empty());

            // Empty fragments preserve the earlier source, just as the terminal writer does.
            let mut fragment = merman_core::diagrams::block::BlockNodeRenderModel::default();
            fragment.id = "A".to_string();
            fragment.block_type = "na".to_string();
            model.blocks_flat.push(fragment);
            let plan =
                resolve_node_paint(Some(&theme), &config, &model, &layout, &work_meter).unwrap();
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert!(evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(evidence.residuals().is_empty());

            // A later non-empty list replaces the corresponding source list.
            let mut fragment = merman_core::diagrams::block::BlockNodeRenderModel::default();
            fragment.id = "A".to_string();
            fragment.block_type = "na".to_string();
            fragment.styles = vec!["stroke:#00bb00".to_string()];
            fragment.classes = vec!["unowned".to_string()];
            model.blocks_flat.push(fragment);
            let plan =
                resolve_node_paint(Some(&theme), &config, &model, &layout, &work_meter).unwrap();
            let evidence = collect_evidence(&theme, |evidence| plan.record_evidence(evidence));
            assert!(!evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(!evidence.residuals().is_empty());
        }
    }

    #[test]
    fn source_all_reset_owns_each_node_class_paint_channel() {
        for id in ["default", "node", "flowchart-label"] {
            let mut classes = IndexMap::new();
            let mut class = BlockClassDefRenderModel::default();
            class.id = id.into();
            class.styles = vec!["all:unset".into()];
            classes.insert(id.into(), class);
            let ownership = BlockNodePaintSourceOwnership::new(&classes);
            assert!(ownership.owns_fill(false, &[]), "class={id}");
            assert!(ownership.owns_stroke(false, &[]), "class={id}");
        }
    }

    #[test]
    fn source_ownership_tracks_fill_and_stroke_independently() {
        let mut class_defs = IndexMap::new();
        let mut accent = BlockClassDefRenderModel::default();
        accent.id = "accent".to_string();
        accent.styles = vec!["fill:#abcdef".to_string()];
        class_defs.insert("accent".to_string(), accent);

        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(true, &[]));
        assert!(ownership.owns_stroke(true, &[]));
        assert!(ownership.owns_fill(false, &["accent".to_string()]));
        assert!(!ownership.owns_stroke(false, &["accent".to_string()]));
        assert!(!ownership.owns_fill(false, &["other".to_string()]));

        let mut default_class = BlockClassDefRenderModel::default();
        default_class.id = "default".to_string();
        default_class.styles = vec!["FILL:#fedcba".to_string(), "STROKE:#abcdef".to_string()];
        class_defs.insert("default".to_string(), default_class);
        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(false, &[]));
        assert!(ownership.owns_stroke(false, &[]));
        assert!(!ownership.owns_fill(false, &["other".to_string()]));
        assert!(!ownership.owns_stroke(false, &["other".to_string()]));
        assert!(ownership.owns_fill(false, &["default".to_string()]));
        assert!(ownership.owns_stroke(false, &["default".to_string()]));
    }
}
