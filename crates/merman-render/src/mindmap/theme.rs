use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::mindmap::{MindmapDiagramRenderEdge, MindmapDiagramRenderModel};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget, ThemeTypographyProperty,
    ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

pub(crate) const MINDMAP_SECTION_COUNT: usize = 11;
const MINDMAP_LABEL_RULE_TARGETS: [ThemeTarget; 2] = [ThemeTarget::Text, ThemeTarget::NodeLabel];
const MINDMAP_THEME_COLOR_LIMIT_DEFAULT: usize = MINDMAP_SECTION_COUNT + 1;
const MINDMAP_COLOR_SCALE_PATHS: [&str; MINDMAP_SECTION_COUNT] = [
    "themeVariables.cScale1",
    "themeVariables.cScale2",
    "themeVariables.cScale3",
    "themeVariables.cScale4",
    "themeVariables.cScale5",
    "themeVariables.cScale6",
    "themeVariables.cScale7",
    "themeVariables.cScale8",
    "themeVariables.cScale9",
    "themeVariables.cScale10",
    "themeVariables.cScale11",
];
const MINDMAP_DEFAULT_COLOR_SCALE: [&str; MINDMAP_THEME_COLOR_LIMIT_DEFAULT] = [
    "hsl(240, 100%, 76.2745098039%)",
    "hsl(60, 100%, 73.5294117647%)",
    "hsl(80, 100%, 76.2745098039%)",
    "hsl(270, 100%, 76.2745098039%)",
    "hsl(300, 100%, 76.2745098039%)",
    "hsl(330, 100%, 76.2745098039%)",
    "hsl(0, 100%, 76.2745098039%)",
    "hsl(30, 100%, 76.2745098039%)",
    "hsl(90, 100%, 76.2745098039%)",
    "hsl(150, 100%, 76.2745098039%)",
    "hsl(180, 100%, 76.2745098039%)",
    "hsl(210, 100%, 76.2745098039%)",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapEdgeStrokeSource {
    ColorScale { section: usize },
    NodeBorder,
}

impl MindmapEdgeStrokeSource {
    fn is_owned(self, config: &MermaidConfig) -> bool {
        let path = match self {
            Self::ColorScale { section } => MINDMAP_COLOR_SCALE_PATHS[section],
            Self::NodeBorder => "themeVariables.nodeBorder",
        };
        merman_core::__private::config_path_overrides_typed_default(config, path)
    }
}

pub(crate) fn mindmap_color_scale_css(config: &MermaidConfig, slot: usize) -> String {
    config
        .get_str(&format!("themeVariables.cScale{slot}"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            MINDMAP_DEFAULT_COLOR_SCALE[slot % MINDMAP_DEFAULT_COLOR_SCALE.len()].to_string()
        })
}

pub(crate) fn mindmap_node_border_css(config: &MermaidConfig) -> String {
    let root_label = config
        .get_str("themeVariables.gitBranchLabel0")
        .unwrap_or("#ffffff");
    config
        .get_str("themeVariables.nodeBorder")
        .unwrap_or(root_label)
        .to_string()
}

pub(crate) fn mindmap_neo_edges_use_node_border(theme: &str) -> bool {
    theme.contains("redux") || theme == "neo-dark"
}

pub(crate) fn mindmap_model_look<'a>(model_look: &'a str, config: &'a MermaidConfig) -> &'a str {
    let model_look = model_look.trim();
    if model_look.is_empty() || model_look == "default" {
        crate::config::mermaid_config_diagram_look(config).as_str()
    } else {
        model_look
    }
}

/// The Mermaid token that owns the final Mindmap node fill at the SVG writer seam.
///
/// The terminal renderer chooses this from its existing look/theme logic. Keeping that decision
/// out of the plan avoids maintaining a second Mindmap theme interpreter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapNodeFillSource {
    ColorScale,
    MainBackground,
}

/// Fixed-size ownership view for the Mermaid tokens that can win Mindmap node fill.
///
/// The renderer computes this once per document so the node loop does not repeatedly scan the
/// same configuration provenance sets.
#[derive(Debug)]
pub(crate) struct MindmapNodeFillOwnership {
    color_scale: [bool; MINDMAP_SECTION_COUNT],
    main_background: bool,
}

impl MindmapNodeFillOwnership {
    pub(crate) fn from_config(config: &MermaidConfig) -> Self {
        Self {
            color_scale: std::array::from_fn(|section| {
                merman_core::__private::config_path_overrides_typed_default(
                    config,
                    MINDMAP_COLOR_SCALE_PATHS[section],
                )
            }),
            main_background: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.mainBkg",
            ),
        }
    }

    fn owns(&self, fill_source: MindmapNodeFillSource, section: usize) -> bool {
        match fill_source {
            MindmapNodeFillSource::ColorScale => self.color_scale[section],
            MindmapNodeFillSource::MainBackground => self.main_background,
        }
    }
}

/// Converts Mermaid's zero-based branch section into the typed Node palette's one-based ordinal.
///
/// The root has no section and is intentionally outside this mapping. Mermaid assigns branch
/// sections in an eleven-slot ring, so defensive inputs above ten retain the renderer's wrap.
const fn node_palette_ordinal_for_section(section: i32) -> Option<usize> {
    if section < 0 {
        None
    } else {
        Some(section as usize % MINDMAP_SECTION_COUNT + 1)
    }
}

fn normalized_section(section: i32) -> Option<usize> {
    node_palette_ordinal_for_section(section).map(|ordinal| ordinal - 1)
}

fn mindmap_theme_color_limit(config: &MermaidConfig) -> usize {
    crate::config::config_f64(config.as_value(), &["themeVariables", "THEME_COLOR_LIMIT"])
        .map(|value| value.round() as i64)
        .filter(|value| *value > 0)
        .unwrap_or(MINDMAP_THEME_COLOR_LIMIT_DEFAULT as i64)
        .clamp(1, 64) as usize
}

fn mindmap_edge_stroke_source(
    config: &MermaidConfig,
    edge: &MindmapDiagramRenderEdge,
) -> Option<MindmapEdgeStrokeSource> {
    let section = edge.section.and_then(normalized_section)?;
    if section + 1 >= mindmap_theme_color_limit(config) {
        return None;
    }
    let look = mindmap_model_look(&edge.look, config);
    let theme = config.get_str("theme").unwrap_or_default();
    if look == "neo" && mindmap_neo_edges_use_node_border(theme) {
        Some(MindmapEdgeStrokeSource::NodeBorder)
    } else {
        Some(MindmapEdgeStrokeSource::ColorScale { section })
    }
}

/// Family-local projection of direct Mindmap theme routes.
///
/// The plan stores the bounded Node palette plus the static Edge stroke winner, the real Mermaid
/// source selected by each edge, and both terminal receipts. The historical type name remains the
/// central artifact seam while the family-local module deepens behind it.
#[derive(Debug)]
pub(crate) struct MindmapNodePalettePlan {
    fills_by_section: [Option<MindmapNodePaletteFill>; MINDMAP_SECTION_COUNT],
    node_fill: Option<DirectPaintExpectation>,
    node_stroke: Option<DirectPaintExpectation>,
    edge_stroke: Option<MindmapEdgeStroke>,
    expected_edges: Vec<MindmapEdgeExpectation>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_node_paint:
        BTreeMap<FamilyThemeMechanismKey, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    pending_edge_stroke_key: Option<FamilyThemeMechanismKey>,
    node_count: usize,
    node_terminal_receipt: OnceLock<MindmapNodePaletteReceipt>,
    edge_terminal_receipt: OnceLock<MindmapEdgeStrokeReceipt>,
    inherited_font_stack: InheritedFontStackPlan,
}

#[derive(Debug)]
struct MindmapNodePaletteFill {
    css: String,
    capability: ThemeCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MindmapEdgeStroke {
    rule_index: usize,
    css: Box<str>,
    capability: ThemeCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MindmapEdgeExpectation {
    id: Box<str>,
    source: Option<MindmapEdgeStrokeSource>,
    source_css: Option<Box<str>>,
    source_owned: bool,
    winning_stroke_rule: Option<usize>,
}

#[derive(Debug, Default)]
struct MindmapEdgeStrokeRuleObservation {
    applicable: bool,
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

#[derive(Debug, Default)]
struct MindmapNodePaintRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

/// The two Mermaid tokens historically supplied by the Mindmap compatibility bridge.
///
/// This is intentionally a writer-local view rather than a cloned `MermaidConfig`: source
/// ownership remains tied to the original effective config, while the SVG writer owns the small
/// legacy-compatible fanout into node, root, edge, and label CSS.
#[derive(Debug)]
pub(crate) struct MindmapWriterThemeTokens {
    main_background_css: Box<str>,
    node_border_css: Box<str>,
    has_direct_node_stroke: bool,
}

impl MindmapWriterThemeTokens {
    pub(crate) fn main_background_css(&self) -> &str {
        &self.main_background_css
    }

    pub(crate) fn node_border_css(&self) -> &str {
        &self.node_border_css
    }

    pub(crate) const fn has_direct_node_stroke(&self) -> bool {
        self.has_direct_node_stroke
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MindmapWriterThemeToken {
    MainBackground,
    NodeBorder,
}

impl MindmapNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        model: &MindmapDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut visited = [false; MINDMAP_SECTION_COUNT];
        for node in &model.nodes {
            work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
            let Some(section) = node.section.and_then(normalized_section) else {
                continue;
            };
            visited[section] = true;
        }

        let mut plan = Self::baseline(config, model, work_meter)?;
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(Some(theme), config);
        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        plan.resolve_node_paint(theme, config, work_meter)?;
        plan.refresh_edge_source_css(config);
        plan.resolve_node_palette(theme, visited, work_meter)?;
        plan.resolve_edge_stroke(theme, work_meter)?;
        let absent = TerminalVariantDomain::uniform(0, ThemeVariant::Default);
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                UnsupportedTerminalDomain::textual(
                    ThemeTarget::NodeLabel,
                    &MINDMAP_LABEL_RULE_TARGETS,
                    TerminalVariantDomain::uniform(model.nodes.len(), ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::direct(ThemeTarget::Title, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::Marker, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::EdgeLabelBackground, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::Cluster, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::ClusterLabel, absent),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Edge,
                    TerminalVariantDomain::uniform(model.edges.len(), ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(model.nodes.len(), ThemeVariant::Default),
                ),
            ],
            work_meter,
        )?;
        Ok(plan)
    }

    fn resolve_node_paint(
        &mut self,
        theme: &ResolvedDiagramTheme,
        config: &MermaidConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.mainBkg",
        );
        let config_owns_stroke = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.nodeBorder",
        );
        let typed_fill = (!config_owns_fill)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten()
            .map(DirectPaintExpectation::from_paint);
        let typed_stroke = (!config_owns_stroke)
            .then(|| {
                resolve_direct_static_stroke(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten()
            .map(DirectPaintExpectation::from_paint);
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();
        if let Some(fill) = typed_fill.as_ref() {
            expected_capabilities.insert(
                (fill.rule_index(), ResolvedStyleProperty::Fill),
                fill.capability(),
            );
        }
        if let Some(stroke) = typed_stroke.as_ref() {
            expected_capabilities.insert(
                (stroke.rule_index(), ResolvedStyleProperty::Stroke),
                stroke.capability(),
            );
        }
        let static_winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let has_ordinal_node_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Node && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        if has_ordinal_node_rules {
            for ordinal in 1..=self.node_count {
                let occurrence_style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                occurrence_winners.extend(
                    occurrence_style
                        .winner_rule_properties()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
            }
        }
        let mut observations = BTreeMap::<usize, MindmapNodePaintRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Node,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !selector.ordinal_domain_intersects_occurrence_count(self.node_count) {
                continue;
            }
            let property = resolved_style_property_for_facet(facet);
            let route_won = match selector {
                FamilyThemeSelectorShape::Static { .. } => {
                    static_winners.contains(&(rule_index, property))
                }
                FamilyThemeSelectorShape::Ordinal { .. } => {
                    occurrence_winners.contains(&(rule_index, property))
                }
            };
            if !route_won {
                continue;
            }

            observation.applicable = true;
            if (config_owns_fill && property == ResolvedStyleProperty::Fill)
                || (config_owns_stroke && property == ResolvedStyleProperty::Stroke)
            {
                continue;
            }
            match (route.disposition(), selector, facet) {
                (
                    FamilyThemeDisposition::TypedAdapter,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                    FamilyThemeRuleFacet::Fill(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                    )
                    | FamilyThemeRuleFacet::Stroke(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                    ),
                ) => {
                    if let Some(capability) = expected_capabilities.get(&(rule_index, property)) {
                        observation.capabilities.insert(property, *capability);
                    } else {
                        observation.incomplete = true;
                    }
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

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Node,
            };
            if !observation.applicable {
                self.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A route with a facet we do not own stays deliberately unaccounted so strict
                // portability fails closed instead of claiming a partial token fanout is enough.
            } else if observation.capabilities.is_empty() {
                self.evidence.mark_not_applicable(key);
            } else {
                self.pending_node_paint
                    .insert(key, observation.capabilities);
            }
        }
        self.node_fill = typed_fill;
        self.node_stroke = typed_stroke;
        Ok(())
    }

    fn refresh_edge_source_css(&mut self, config: &MermaidConfig) {
        let node_border_css = self
            .node_stroke
            .as_ref()
            .map(|stroke| stroke.css().to_owned())
            .unwrap_or_else(|| mindmap_node_border_css(config));
        for edge in &mut self.expected_edges {
            edge.source_css = edge.source.map(|source| match source {
                MindmapEdgeStrokeSource::ColorScale { section } => {
                    mindmap_color_scale_css(config, section + 1).into_boxed_str()
                }
                MindmapEdgeStrokeSource::NodeBorder => node_border_css.clone().into_boxed_str(),
            });
        }
    }

    fn resolve_node_palette(
        &mut self,
        theme: &ResolvedDiagramTheme,
        visited: [bool; MINDMAP_SECTION_COUNT],
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::Node) else {
            return Ok(());
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };
        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                self.palette_key = Some(key.clone());
                for (section, present) in visited.into_iter().enumerate() {
                    if !present {
                        continue;
                    }
                    let ordinal = section + 1;
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    if !matches!(style.fill_resolution().specified(), Specified::Unspecified) {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::Node, ordinal) else {
                        self.evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                        self.palette_key = None;
                        self.fills_by_section = std::array::from_fn(|_| None);
                        return Ok(());
                    };
                    self.fills_by_section[section] = Some(MindmapNodePaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    });
                }
            }
            FamilyThemeDisposition::Unsupported => {
                self.evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
            FamilyThemeDisposition::LegacyCompatibility => {}
        }
        Ok(())
    }

    fn resolve_edge_stroke(
        &mut self,
        theme: &ResolvedDiagramTheme,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let edge_count = self.expected_edges.len();
        let static_style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_stroke = typed_static_edge_stroke(theme, &static_style);
        let static_stroke_rule = static_stroke.as_ref().map(|stroke| stroke.rule_index);
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();

        if edge_count == 0 {
            winner_properties.extend(
                static_style
                    .winner_rule_properties()
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
        } else {
            for (edge_index, expectation) in self.expected_edges.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    Some(edge_index + 1),
                    work_meter,
                )?;
                winner_properties.extend(
                    style
                        .winner_rule_properties()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
                expectation.winning_stroke_rule = style
                    .stroke_resolution()
                    .winner()
                    .map(|origin| origin.rule_index());
            }
        }

        let mut observations = BTreeMap::<usize, MindmapEdgeStrokeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let empty_direct_candidate = edge_count == 0
                        && static_stroke_rule == Some(rule_index)
                        && matches!(
                            (route.disposition(), selector, facet),
                            (
                                FamilyThemeDisposition::TypedAdapter,
                                FamilyThemeSelectorShape::Static { variant: None },
                                FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                                ),
                            )
                        );
                    if !empty_direct_candidate
                        && !selector.ordinal_domain_intersects_occurrence_count(edge_count)
                    {
                        continue;
                    }
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    if !route_won && !empty_direct_candidate {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if static_stroke_rule == Some(rule_index) => {
                            observation.pending = true;
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
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
                target: ThemeTarget::Edge,
            };
            if !observation.applicable {
                self.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                self.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed or selector-qualified rule stays fail-closed until every winning facet
                // has a family-owned terminal consumer.
            } else if observation.pending {
                debug_assert_eq!(static_stroke_rule, Some(rule_index));
                debug_assert!(self.pending_edge_stroke_key.is_none());
                self.pending_edge_stroke_key = Some(key);
            } else {
                self.evidence.mark_not_applicable(key);
            }
        }

        if self.pending_edge_stroke_key.is_some() {
            self.edge_stroke = static_stroke;
        }
        Ok(())
    }

    fn baseline(
        config: &MermaidConfig,
        model: &MindmapDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
        let expected_edges = model
            .edges
            .iter()
            .map(|edge| {
                work_meter.checkpoint(merman_core::OperationPhase::Layout)?;
                let source = mindmap_edge_stroke_source(config, edge);
                Ok(MindmapEdgeExpectation {
                    id: edge.id.clone().into_boxed_str(),
                    source,
                    source_css: None,
                    source_owned: source.is_some_and(|source| source.is_owned(config)),
                    winning_stroke_rule: None,
                })
            })
            .collect::<Result<_, OperationWorkError>>()?;
        Ok(Self {
            fills_by_section: std::array::from_fn(|_| None),
            node_fill: None,
            node_stroke: None,
            edge_stroke: None,
            expected_edges,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            pending_node_paint: BTreeMap::new(),
            pending_edge_stroke_key: None,
            node_count: model.nodes.len(),
            node_terminal_receipt: OnceLock::new(),
            edge_terminal_receipt: OnceLock::new(),
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(None, config),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn terminal_decision_for_node(
        &self,
        section: Option<i32>,
        fill_source: MindmapNodeFillSource,
        ownership: &MindmapNodeFillOwnership,
        generated_color_rule_limit: usize,
    ) -> MindmapNodePaletteTerminalDecision {
        let Some(section) = section.and_then(normalized_section) else {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        };
        let Some(fill) = self.fills_by_section[section].as_ref() else {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        };
        if section + 1 >= generated_color_rule_limit {
            return MindmapNodePaletteTerminalDecision::Unsupported;
        }
        if ownership.owns(fill_source, section) {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        }
        MindmapNodePaletteTerminalDecision::Applied {
            section,
            capability: fill.capability,
        }
    }

    pub(crate) fn css_fill_for_section(
        &self,
        section: usize,
        generated_color_rule_limit: usize,
    ) -> Option<&str> {
        if section >= MINDMAP_SECTION_COUNT || section + 1 >= generated_color_rule_limit {
            return None;
        }
        self.fills_by_section[section]
            .as_ref()
            .map(|fill| fill.css.as_str())
    }

    pub(crate) fn writer_theme_tokens(
        &self,
        main_background_css: &str,
        node_border_css: &str,
    ) -> MindmapWriterThemeTokens {
        MindmapWriterThemeTokens {
            main_background_css: self
                .node_fill
                .as_ref()
                .map(|fill| fill.css().into())
                .unwrap_or_else(|| main_background_css.into()),
            node_border_css: self
                .node_stroke
                .as_ref()
                .map(|stroke| stroke.css().into())
                .unwrap_or_else(|| node_border_css.into()),
            has_direct_node_stroke: self.node_stroke.is_some(),
        }
    }

    pub(crate) fn begin_terminal_receipt(&self) -> MindmapNodePaletteReceipt {
        MindmapNodePaletteReceipt::new(
            self.node_count,
            self.font_family_css(),
            self.node_fill.clone(),
            self.node_stroke.clone(),
        )
    }

    pub(crate) fn record_terminal(&self, receipt: MindmapNodePaletteReceipt) -> bool {
        receipt.proves_complete() && self.node_terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn terminal_edge_stroke(&self, edge_index: usize) -> Option<(usize, &str)> {
        let stroke = self.edge_stroke.as_ref()?;
        let expectation = self.expected_edges.get(edge_index)?;
        (expectation.winning_stroke_rule == Some(stroke.rule_index) && !expectation.source_owned)
            .then_some((stroke.rule_index, stroke.css.as_ref()))
    }

    pub(crate) fn terminal_edge_source(
        &self,
        edge_index: usize,
    ) -> Option<MindmapEdgeStrokeSource> {
        self.expected_edges
            .get(edge_index)
            .and_then(|expectation| expectation.source)
    }

    pub(crate) fn begin_edge_terminal_receipt(&self) -> Option<MindmapEdgeStrokeReceipt> {
        self.pending_edge_stroke_key
            .is_some()
            .then(|| MindmapEdgeStrokeReceipt::new(self))
    }

    pub(crate) fn record_edge_terminal(&self, receipt: MindmapEdgeStrokeReceipt) -> bool {
        self.pending_edge_stroke_key.is_some()
            && receipt.proves_complete(self)
            && self.edge_terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let has_visible_typography = self.node_count != 0;
        for (key, properties) in &self.pending_node_paint {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography(_)
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            let Some(receipt) = self.node_terminal_receipt.get() else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                continue;
            };
            if properties
                .keys()
                .all(|property| receipt.proves_node_paint_property(rule_index, *property))
            {
                evidence.mark_applied_with_capabilities(key.clone(), properties.values().copied());
            } else if receipt.proves_complete()
                && properties.iter().all(|(property, _)| {
                    !receipt.has_effective_node_paint_property(rule_index, *property)
                })
            {
                evidence.mark_not_applicable(key.clone());
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_typography);
        if let Some(key) = self.palette_key.clone() {
            match self.node_terminal_receipt.get() {
                Some(receipt) if receipt.unsupported_surface => evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
                Some(receipt) if !receipt.applied_capabilities.is_empty() => {
                    evidence.mark_applied_with_capabilities(
                        key,
                        receipt.applied_capabilities.iter().copied(),
                    );
                }
                Some(_) => evidence.mark_not_applicable(key),
                None => evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            }
        }
        if let (Some(key), Some(stroke)) = (
            self.pending_edge_stroke_key.clone(),
            self.edge_stroke.as_ref(),
        ) {
            match self.edge_terminal_receipt.get() {
                Some(receipt) if receipt.proves_rule(self, stroke.rule_index) => {
                    evidence.mark_applied_with_capabilities(key, [stroke.capability]);
                }
                Some(receipt) if receipt.proves_complete(self) => {
                    evidence.mark_not_applicable(key);
                }
                Some(_) | None => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }
        if self.inherited_font_stack.typed_font_stack_requested() {
            let typography_key =
                FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed if has_visible_typography => {
                    match self.node_terminal_receipt.get() {
                        Some(receipt) if receipt.proves_font_stack(self.font_family_css()) => {
                            evidence.mark_applied_with_capabilities(
                                typography_key,
                                [ThemeCapability::Typography],
                            );
                        }
                        _ => evidence.mark_residual(
                            typography_key,
                            FamilyThemeResidualReason::UnsupportedTypography,
                        ),
                    }
                }
                InheritedFontStackOutcome::Typed => evidence.mark_not_applicable(typography_key),
                InheritedFontStackOutcome::ConfigOwned => {
                    evidence.mark_not_applicable(typography_key)
                }
                InheritedFontStackOutcome::Unsupported => evidence.mark_residual(
                    typography_key,
                    FamilyThemeResidualReason::UnsupportedTypography,
                ),
                InheritedFontStackOutcome::Inactive => {}
            }
        }
        evidence
    }
}

fn typed_static_edge_stroke(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<MindmapEdgeStroke> {
    let origin = style.stroke_resolution().winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if rule.variant().is_some()
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    match style.stroke_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => Some(MindmapEdgeStroke {
            rule_index: origin.rule_index(),
            css: "transparent".into(),
            capability: ThemeCapability::TransparentPaint,
        }),
        Specified::Value(CanvasPaint::Solid(color)) => Some(MindmapEdgeStroke {
            rule_index: origin.rule_index(),
            css: color.as_css().into_boxed_str(),
            capability: ThemeCapability::SolidPaint,
        }),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => None,
    }
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style.split(';').find_map(|declaration| {
        let (property, value) = declaration.split_once(':')?;
        if property.trim() != "stroke" {
            return None;
        }
        Some(
            value
                .trim()
                .strip_suffix("!important")
                .unwrap_or_else(|| value.trim())
                .trim(),
        )
    })
}

/// Writer-owned proof that every Mindmap edge path reached the final SVG sink in semantic order
/// with the exact family-selected or Mermaid-owned stroke value.
#[derive(Debug)]
pub(crate) struct MindmapEdgeStrokeReceipt {
    expected_edge_count: usize,
    next_edge: usize,
    values_match: bool,
    applied_rule_indices: BTreeSet<usize>,
}

impl MindmapEdgeStrokeReceipt {
    fn new(plan: &MindmapNodePalettePlan) -> Self {
        Self {
            expected_edge_count: plan.expected_edges.len(),
            next_edge: 0,
            values_match: true,
            applied_rule_indices: BTreeSet::new(),
        }
    }

    pub(crate) fn record_edge(
        &mut self,
        plan: &MindmapNodePalettePlan,
        edge_index: usize,
        edge_id: &str,
        source: Option<MindmapEdgeStrokeSource>,
        final_stroke: Option<&str>,
        terminal_style: Option<&str>,
    ) {
        let Some(expected) = plan.expected_edges.get(self.next_edge) else {
            self.values_match = false;
            return;
        };
        self.values_match &= edge_index == self.next_edge
            && edge_index < self.expected_edge_count
            && expected.id.as_ref() == edge_id
            && expected.source == source;

        match plan.terminal_edge_stroke(edge_index) {
            Some((rule_index, css)) => {
                self.values_match &= final_stroke == Some(css)
                    && terminal_style.and_then(terminal_stroke) == Some(css);
                self.applied_rule_indices.insert(rule_index);
            }
            None => {
                self.values_match &= final_stroke == expected.source_css.as_deref()
                    && terminal_style.and_then(terminal_stroke).is_none();
            }
        }
        self.next_edge = self.next_edge.saturating_add(1);
    }

    fn proves_complete(&self, plan: &MindmapNodePalettePlan) -> bool {
        self.values_match
            && self.expected_edge_count == plan.expected_edges.len()
            && self.next_edge == self.expected_edge_count
    }

    fn proves_rule(&self, plan: &MindmapNodePalettePlan, rule_index: usize) -> bool {
        self.proves_complete(plan) && self.applied_rule_indices.contains(&rule_index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapNodePaletteTerminalDecision {
    Applied {
        section: usize,
        capability: ThemeCapability,
    },
    NotApplicable,
    Unsupported,
}

/// Writer-owned proof that every semantic Mindmap node reached its canonical shape checkpoint.
#[derive(Debug)]
pub(crate) struct MindmapNodePaletteReceipt {
    expected_node_count: usize,
    checkpointed_node_count: usize,
    node_order_matches: bool,
    unsupported_surface: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
    expected_font_family_css: Box<str>,
    font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    expected_node_fill: Option<DirectPaintExpectation>,
    expected_node_stroke: Option<DirectPaintExpectation>,
    main_background_css: Option<Box<str>>,
    node_border_css: Option<Box<str>>,
    node_fill_css_matches: bool,
    node_stroke_css_matches: bool,
    node_paint_ledger: DirectPaintTerminalLedger,
}

impl MindmapNodePaletteReceipt {
    fn new(
        node_count: usize,
        expected_font_family_css: &str,
        expected_node_fill: Option<DirectPaintExpectation>,
        expected_node_stroke: Option<DirectPaintExpectation>,
    ) -> Self {
        Self {
            expected_node_count: node_count,
            checkpointed_node_count: 0,
            node_order_matches: true,
            unsupported_surface: false,
            applied_capabilities: BTreeSet::new(),
            expected_font_family_css: expected_font_family_css.into(),
            font_family_css: None,
            css_emission_unique: true,
            expected_node_fill,
            expected_node_stroke,
            main_background_css: None,
            node_border_css: None,
            node_fill_css_matches: true,
            node_stroke_css_matches: true,
            node_paint_ledger: DirectPaintTerminalLedger::default(),
        }
    }

    pub(crate) fn record_typography_css(&mut self, emitted_font_family_css: &str) {
        if self.font_family_css.is_some() {
            self.css_emission_unique = false;
            return;
        }
        self.css_emission_unique =
            emitted_font_family_css == self.expected_font_family_css.as_ref();
        self.font_family_css = Some(emitted_font_family_css.into());
    }

    pub(crate) fn record_node_paint_css(
        &mut self,
        main_background_css: &str,
        node_border_css: &str,
    ) {
        if self.expected_node_fill.is_some() {
            if self.main_background_css.is_some() {
                self.node_fill_css_matches = false;
            } else {
                self.node_fill_css_matches &= self
                    .expected_node_fill
                    .as_ref()
                    .is_some_and(|expected| expected.css() == main_background_css);
                self.main_background_css = Some(main_background_css.into());
            }
        }
        if self.expected_node_stroke.is_some() {
            if self.node_border_css.is_some() {
                self.node_stroke_css_matches = false;
            } else {
                self.node_stroke_css_matches &= self
                    .expected_node_stroke
                    .as_ref()
                    .is_some_and(|expected| expected.css() == node_border_css);
                self.node_border_css = Some(node_border_css.into());
            }
        }
    }

    pub(crate) fn record_node_paint_terminal(
        &mut self,
        token: MindmapWriterThemeToken,
        emitted_css: &str,
    ) {
        let (expected, property, emitted_from_css) = match token {
            MindmapWriterThemeToken::MainBackground => (
                self.expected_node_fill.as_ref(),
                ResolvedStyleProperty::Fill,
                self.main_background_css.as_deref(),
            ),
            MindmapWriterThemeToken::NodeBorder => (
                self.expected_node_stroke.as_ref(),
                ResolvedStyleProperty::Stroke,
                self.node_border_css.as_deref(),
            ),
        };
        let Some(expected) = expected else {
            return;
        };
        let matches = emitted_from_css == Some(emitted_css)
            && self.node_paint_ledger.record(
                Some(expected),
                false,
                Some((expected.rule_index(), emitted_css)),
                property,
            );
        match property {
            ResolvedStyleProperty::Fill => self.node_fill_css_matches &= matches,
            ResolvedStyleProperty::Stroke => self.node_stroke_css_matches &= matches,
            _ => unreachable!("Mindmap paint receipt property"),
        }
    }

    pub(crate) fn record_checkpointed_node(
        &mut self,
        node_index: usize,
        terminal_decision: MindmapNodePaletteTerminalDecision,
    ) {
        if node_index != self.checkpointed_node_count || node_index >= self.expected_node_count {
            self.node_order_matches = false;
            return;
        }
        self.checkpointed_node_count += 1;
        match terminal_decision {
            MindmapNodePaletteTerminalDecision::Applied { capability, .. } => {
                self.applied_capabilities.insert(capability);
            }
            MindmapNodePaletteTerminalDecision::Unsupported => self.unsupported_surface = true,
            MindmapNodePaletteTerminalDecision::NotApplicable => {}
        }
    }

    fn proves_complete(&self) -> bool {
        self.node_order_matches && self.checkpointed_node_count == self.expected_node_count
    }

    fn has_effective_node_paint_property(
        &self,
        rule_index: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.node_paint_ledger
            .has_effective_property(rule_index, property)
    }

    fn proves_node_paint_property(
        &self,
        rule_index: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.proves_complete()
            && match property {
                ResolvedStyleProperty::Fill => self.node_fill_css_matches,
                ResolvedStyleProperty::Stroke => self.node_stroke_css_matches,
                _ => false,
            }
            && self
                .node_paint_ledger
                .proves_property(self.proves_complete(), rule_index, property)
    }

    fn proves_font_stack(&self, expected_font_family_css: &str) -> bool {
        self.proves_complete()
            && self.css_emission_unique
            && !expected_font_family_css.trim().is_empty()
            && self.font_family_css.as_deref() == Some(expected_font_family_css)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, OrdinalSelector,
        ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use serde_json::{Value, json};

    fn effective_config(site_config: Value) -> MermaidConfig {
        merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(site_config))
            .parse_metadata_sync("mindmap\n  root\n    child\n")
            .expect("parse Mindmap config fixture")
            .effective_config
    }

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn model_with_sections(
        sections: impl IntoIterator<Item = Option<i32>>,
    ) -> MindmapDiagramRenderModel {
        let nodes = sections
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
                json!({
                    "id": index.to_string(),
                    "domId": format!("node_{index}"),
                    "label": format!("node {index}"),
                    "shape": "defaultMindmapNode",
                    "cssClasses": "node",
                    "section": section,
                })
            })
            .collect::<Vec<_>>();
        serde_json::from_value(json!({ "nodes": nodes, "edges": [] }))
            .expect("Mindmap theme-plan fixture")
    }

    fn resolved_node_palette(colors: &[&str], static_fill: Option<&str>) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(*color).expect("valid test color")),
        )
        .expect("non-empty test palette");
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette);
        if let Some(fill) = static_fill {
            styles = styles.with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid(fill).expect("valid static Mindmap Node fill"),
                    ),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Mindmap Node palette")
            .resolve(DiagramFamilyId::MINDMAP)
    }

    #[test]
    fn palette_plan_skips_root_and_maps_branch_sections() {
        let theme = resolved_node_palette(&["#ef4444", "#2563eb"], None);
        let config = effective_config(json!({}));
        let model = model_with_sections([None, Some(-1), Some(0), Some(1), Some(10), Some(11)]);
        let plan = MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &work_meter())
            .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(-1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 1,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(10),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 10,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(11),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.css_fill_for_section(0, MINDMAP_SECTION_COUNT + 1),
            Some("#ef4444")
        );
        assert_eq!(
            plan.css_fill_for_section(1, MINDMAP_SECTION_COUNT + 1),
            Some("#2563eb")
        );
    }

    #[test]
    fn static_node_fill_winner_blocks_palette_fallback() {
        let theme = resolved_node_palette(&["#ef4444"], Some("#111827"));
        let config = effective_config(json!({}));
        let model = model_with_sections([Some(0), Some(1)]);
        let plan = MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &work_meter())
            .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
    }

    #[test]
    fn transparent_palette_color_reports_transparent_paint() {
        let theme = resolved_node_palette(&["transparent"], None);
        let config = effective_config(json!({}));
        let model = model_with_sections([Some(0)]);
        let plan = MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &work_meter())
            .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::TransparentPaint,
            }
        );
    }

    #[test]
    fn visible_label_terminal_reconciles_text_and_node_label_winners() {
        let styles = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#111111").unwrap()),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#222222").unwrap()),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Mindmap label terminal fixture")
            .resolve(DiagramFamilyId::MINDMAP);
        let config = effective_config(json!({}));
        let model = model_with_sections([None]);

        let evidence =
            MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &work_meter())
                .expect("resolve Mindmap label terminal evidence")
                .finish_evidence();

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            }]
        );
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::NodeLabel,
            }
        );
    }

    #[test]
    fn terminal_less_mindmap_title_is_not_applicable() {
        let styles = ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .for_family(DiagramFamilyId::MINDMAP),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile terminal-less Mindmap title fixture")
            .resolve(DiagramFamilyId::MINDMAP);
        let config = effective_config(json!({}));
        let model = model_with_sections([None]);

        let evidence =
            MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &work_meter())
                .expect("resolve terminal-less Mindmap title evidence")
                .finish_evidence();

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Title,
            }]
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn baseline_palette_observes_cancelled_operations_without_a_theme() {
        let control = merman_core::OperationControl::new();
        control.cancel();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        let config = effective_config(json!({}));
        let model = model_with_sections([Some(0), Some(1)]);
        let error = MindmapNodePalettePlan::resolve(None, &config, &model, &meter)
            .expect_err("baseline preparation must observe operation cancellation");
        assert!(matches!(error, OperationWorkError::Cancelled(_)));
    }

    #[test]
    fn palette_plan_has_an_exact_ordinal_work_boundary() {
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#ef4444").unwrap()]).unwrap();
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Node, palette)
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#111827").unwrap()),
                )
                .for_family(DiagramFamilyId::MINDMAP)
                .with_ordinal(OrdinalSelector::exact(99).unwrap()),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile metered Mindmap palette")
            .resolve(DiagramFamilyId::MINDMAP);
        let config = effective_config(json!({}));
        let model = model_with_sections([Some(0), Some(1)]);
        let unbounded = work_meter();

        MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &unbounded)
            .expect("resolve unbounded Mindmap palette plan");
        let exact = unbounded.used();
        assert!(exact > 0);

        let exact_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact)
                .unwrap(),
        );
        MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &exact_meter)
            .expect("exact Mindmap ordinal work budget must succeed");
        assert_eq!(exact_meter.used(), exact);

        let short_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        );
        let error = MindmapNodePalettePlan::resolve(Some(&theme), &config, &model, &short_meter)
            .expect_err("one-unit-short Mindmap budget must fail closed");
        let OperationWorkError::ResourceLimitExceeded(error) = error else {
            panic!("expected structured layout work rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, exact);
        assert_eq!(error.max, exact - 1);
        assert!(short_meter.used() <= exact - 1);
    }
}
