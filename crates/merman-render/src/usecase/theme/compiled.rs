//! Family-owned theme terminals shared by Usecase measurement and SVG emission.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

use indexmap::IndexMap;
use merman_model::families::usecase::{
    UsecaseArrowType, UsecaseDiagramModel, UsecaseNodeKind, UsecaseRelationshipType,
};
use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect,
    ResolvedThemeStyle, Specified, SvgShadowEffect, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, resolve_direct_static_fill, resolve_direct_static_stroke,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;
use crate::text::TextStyle;

use super::UsecaseCssBinding;

#[derive(Debug, Clone)]
struct Observation {
    key: FamilyThemeMechanismKey,
    capability: Option<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

/// A typed terminal, never a JSON config overlay or a serialized SVG edit.
#[derive(Debug, Clone, Default)]
pub(crate) struct UsecaseThemeStyle {
    terminal: Option<usize>,
    pub(crate) fill: Option<String>,
    pub(crate) stroke: Option<String>,
    pub(crate) stroke_width: Option<f64>,
    dasharray: Option<String>,
    linecap: Option<&'static str>,
    linejoin: Option<&'static str>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: Option<f64>,
    pub(crate) shadow: Option<SvgShadowEffect>,
    clear_effect: bool,
}

impl UsecaseThemeStyle {
    pub(crate) fn text_style(&self, inherited: &TextStyle) -> TextStyle {
        let mut style = inherited.clone();
        if let Some(family) = &self.font_family {
            style.font_family = Some(family.clone());
        }
        if let Some(size) = self.font_size {
            style.font_size = size;
        }
        style
    }

    pub(crate) fn css(&self, textual: bool) -> String {
        let mut css = String::new();
        if let Some(fill) = &self.fill {
            let _ = write!(css, "fill:{fill} !important;");
            if textual {
                let _ = write!(css, "color:{fill} !important;");
            }
        }
        if let Some(stroke) = &self.stroke {
            let _ = write!(css, "stroke:{stroke} !important;");
        }
        if let Some(width) = self.stroke_width {
            let _ = write!(css, "stroke-width:{width}px !important;");
        }
        if let Some(dasharray) = &self.dasharray {
            let _ = write!(css, "stroke-dasharray:{dasharray} !important;");
        }
        if let Some(linecap) = self.linecap {
            let _ = write!(css, "stroke-linecap:{linecap} !important;");
        }
        if let Some(linejoin) = self.linejoin {
            let _ = write!(css, "stroke-linejoin:{linejoin} !important;");
        }
        if let Some(family) = &self.font_family {
            let _ = write!(css, "font-family:{family} !important;");
        }
        if let Some(size) = self.font_size {
            let _ = write!(css, "font-size:{size}px !important;");
        }
        if self.clear_effect {
            css.push_str("filter:none !important;");
        }
        css
    }

    pub(crate) fn note_path_css(&self, outline: bool) -> String {
        let mut style = self.clone();
        if outline {
            style.fill = None;
        } else {
            style.stroke = None;
            style.stroke_width = None;
            style.dasharray = None;
            style.linecap = None;
            style.linejoin = None;
        }
        style.css(false)
    }

    pub(crate) fn record_emitted(&self, receipt: &UsecaseThemeReceipt) {
        if let Some(index) = self.terminal {
            receipt.emitted.borrow_mut()[index] = true;
        }
    }

    pub(crate) fn record_shadow_emitted(&self, receipt: &UsecaseThemeReceipt) {
        if let Some(index) = self.terminal {
            receipt.effects.borrow_mut().insert(index);
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct UsecaseThemeReceipt {
    emitted: RefCell<Vec<bool>>,
    effects: RefCell<BTreeSet<usize>>,
}

#[derive(Debug)]
struct UsecaseThemeSeal {
    emitted: Vec<bool>,
    effects: BTreeSet<usize>,
}

#[derive(Debug, Default)]
struct Outcome {
    applied: bool,
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

#[derive(Debug, Default)]
pub(crate) struct UsecaseThemePlan {
    theme: Option<ResolvedDiagramTheme>,
    shells: BTreeMap<String, UsecaseThemeStyle>,
    labels: BTreeMap<String, UsecaseThemeStyle>,
    stereotypes: BTreeMap<String, UsecaseThemeStyle>,
    edges: BTreeMap<String, UsecaseThemeStyle>,
    edge_labels: BTreeMap<String, UsecaseThemeStyle>,
    edge_backgrounds: BTreeMap<String, UsecaseThemeStyle>,
    markers: BTreeMap<(String, bool), UsecaseThemeStyle>,
    title: UsecaseThemeStyle,
    terminals: Vec<Vec<Observation>>,
    receipt: OnceLock<UsecaseThemeSeal>,
}

struct Surface<'a> {
    target: ThemeTarget,
    ordinal: usize,
    textual: bool,
    has_fill: bool,
    source: &'a IndexMap<String, String>,
    fill_paths: &'a [&'a str],
    stroke_paths: &'a [&'a str],
    font_prefix: Option<&'a str>,
    fill_default: &'a str,
    stroke_default: &'a str,
    shadow: bool,
}

impl UsecaseThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
        model: &UsecaseDiagramModel,
        diagram_title: Option<&str>,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut plan = Self {
            theme: Some(theme.clone()),
            ..Self::default()
        };
        let binding = UsecaseCssBinding::new(config.as_value());
        let html = crate::config::config_bool(config.as_value(), &["htmlLabels"]).unwrap_or(true);
        let mut counts = BTreeMap::<ThemeTarget, usize>::new();
        let mut ordinal = |target| {
            let count = counts.entry(target).or_default();
            *count += 1;
            *count
        };
        for node in &model.nodes {
            work.charge(1)?;
            let actor = node.kind == UsecaseNodeKind::Actor;
            let target = if actor {
                ThemeTarget::Actor
            } else {
                ThemeTarget::Node
            };
            let label_target = if actor {
                ThemeTarget::ActorLabel
            } else {
                ThemeTarget::NodeLabel
            };
            let separate_stereotype =
                actor || (node.business && node.kind == UsecaseNodeKind::UseCaseEllipse);
            let role_font = html || separate_stereotype;
            let source = source_styles(model, &node.classes, &node.styles);
            let index = ordinal(target);
            let shell = plan.resolve_surface(
                config,
                Surface {
                    target,
                    ordinal: index,
                    textual: false,
                    has_fill: true,
                    source: &source,
                    fill_paths: if actor { ACTOR_FILL } else { NODE_FILL },
                    stroke_paths: if actor { ACTOR_STROKE } else { NODE_STROKE },
                    font_prefix: None,
                    fill_default: if actor { &binding.actor } else { &binding.body },
                    stroke_default: if actor {
                        &binding.actor_border
                    } else {
                        &binding.border
                    },
                    shadow: !actor && binding.look == "classic" && bounded_body_source(&source),
                },
                work,
            )?;
            plan.shells.insert(node.id.clone(), shell);
            let has_stereotype = node
                .stereotype
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty());
            if !node.label.trim().is_empty() || (has_stereotype && !separate_stereotype) {
                let label = plan.resolve_surface(
                    config,
                    Surface {
                        target: label_target,
                        ordinal: index,
                        textual: true,
                        has_fill: true,
                        source: &source,
                        fill_paths: if actor { ACTOR_TEXT } else { NODE_TEXT },
                        stroke_paths: &[],
                        font_prefix: role_font.then_some(if actor { "actor" } else { "usecase" }),
                        fill_default: if actor {
                            &binding.actor_text
                        } else {
                            &binding.text
                        },
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                plan.labels.insert(node.id.clone(), label);
            }
            if has_stereotype && separate_stereotype {
                let label = plan.resolve_surface(
                    config,
                    Surface {
                        target: label_target,
                        ordinal: index,
                        textual: true,
                        has_fill: true,
                        source: &source,
                        fill_paths: if html && actor { ACTOR_TEXT } else { NODE_TEXT },
                        stroke_paths: &[],
                        font_prefix: html.then_some(if actor { "actor" } else { "usecase" }),
                        fill_default: if html && actor {
                            &binding.actor_text
                        } else {
                            &binding.text
                        },
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                plan.stereotypes.insert(node.id.clone(), label);
            }
        }
        for node in &model.json_nodes {
            work.charge(1)?;
            let source = source_styles(model, &node.classes, &node.styles);
            let index = ordinal(ThemeTarget::Node);
            let shell = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Node,
                    ordinal: index,
                    textual: false,
                    has_fill: true,
                    source: &source,
                    fill_paths: JSON_FILL,
                    stroke_paths: JSON_STROKE,
                    font_prefix: None,
                    fill_default: &binding.main,
                    stroke_default: &binding.node_border,
                    shadow: false,
                },
                work,
            )?;
            let label = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::NodeLabel,
                    ordinal: index,
                    textual: true,
                    has_fill: true,
                    source: &source,
                    fill_paths: NODE_TEXT,
                    stroke_paths: &[],
                    font_prefix: None,
                    fill_default: &binding.text,
                    stroke_default: "none",
                    shadow: false,
                },
                work,
            )?;
            plan.shells.insert(node.id.clone(), shell);
            plan.labels.insert(node.id.clone(), label);
        }
        for note in &model.notes {
            work.charge(1)?;
            let source = source_styles(model, &[], &[]);
            let index = ordinal(ThemeTarget::Note);
            let shell = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Note,
                    ordinal: index,
                    textual: false,
                    has_fill: true,
                    source: &source,
                    fill_paths: &["themeVariables.noteBkgColor"],
                    stroke_paths: &["themeVariables.noteBorderColor"],
                    font_prefix: None,
                    fill_default: &binding.note,
                    stroke_default: &binding.note_border,
                    shadow: false,
                },
                work,
            )?;
            plan.shells.insert(note.id.clone(), shell);
            if !note.label.trim().is_empty() {
                let label = plan.resolve_surface(
                    config,
                    Surface {
                        target: ThemeTarget::NoteLabel,
                        ordinal: index,
                        textual: true,
                        has_fill: true,
                        source: &source,
                        fill_paths: &[
                            "themeVariables.noteTextColor",
                            "themeVariables.primaryTextColor",
                        ],
                        stroke_paths: &[],
                        font_prefix: None,
                        fill_default: &binding.note_text,
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                plan.labels.insert(note.id.clone(), label);
            }
        }
        for boundary in &model.boundaries {
            work.charge(1)?;
            let source = source_styles(model, &boundary.classes, &boundary.styles);
            let index = ordinal(ThemeTarget::Cluster);
            let shell = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Cluster,
                    ordinal: index,
                    textual: false,
                    has_fill: true,
                    source: &source,
                    fill_paths: &[
                        "themeVariables.usecaseBoundaryBkg",
                        "themeVariables.clusterBkg",
                    ],
                    stroke_paths: &[
                        "themeVariables.usecaseBoundaryBorder",
                        "themeVariables.clusterBorder",
                    ],
                    font_prefix: None,
                    fill_default: &binding.boundary,
                    stroke_default: &binding.boundary_border,
                    shadow: false,
                },
                work,
            )?;
            plan.shells.insert(boundary.id.clone(), shell);
            if !boundary.label.trim().is_empty() {
                let label = plan.resolve_surface(
                    config,
                    Surface {
                        target: ThemeTarget::ClusterLabel,
                        ordinal: index,
                        textual: true,
                        has_fill: true,
                        source: &source,
                        fill_paths: &[
                            "themeVariables.titleColor",
                            "themeVariables.primaryTextColor",
                        ],
                        stroke_paths: &[],
                        font_prefix: None,
                        fill_default: &binding.title,
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                plan.labels.insert(boundary.id.clone(), label);
            }
        }
        for edge in &model.relationships {
            work.charge(1)?;
            let source = source_styles(model, &edge.classes, &edge.styles);
            let index = ordinal(ThemeTarget::Edge);
            let stroke_paths = match edge.relationship_type {
                UsecaseRelationshipType::Include => &[
                    "themeVariables.usecaseIncludeLine",
                    "themeVariables.lineColor",
                ][..],
                UsecaseRelationshipType::Extend => &[
                    "themeVariables.usecaseExtendLine",
                    "themeVariables.lineColor",
                ][..],
                _ => &["themeVariables.lineColor"][..],
            };
            let shell = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Edge,
                    ordinal: index,
                    textual: false,
                    has_fill: true,
                    source: &source,
                    fill_paths: &[],
                    stroke_paths,
                    font_prefix: None,
                    fill_default: "none",
                    stroke_default: &binding.line,
                    shadow: false,
                },
                work,
            )?;
            plan.edges.insert(edge.id.clone(), shell);
            let labelled = matches!(
                edge.relationship_type,
                UsecaseRelationshipType::Include | UsecaseRelationshipType::Extend
            ) || (edge.relationship_type == UsecaseRelationshipType::Association
                && edge
                    .label
                    .as_ref()
                    .is_some_and(|text| !text.trim().is_empty()));
            if labelled {
                let label = plan.resolve_surface(
                    config,
                    Surface {
                        target: ThemeTarget::EdgeLabel,
                        ordinal: index,
                        textual: true,
                        has_fill: true,
                        source: &source,
                        fill_paths: NODE_TEXT,
                        stroke_paths: &[],
                        font_prefix: None,
                        fill_default: &binding.text,
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                let background = plan.resolve_surface(
                    config,
                    Surface {
                        target: ThemeTarget::EdgeLabelBackground,
                        ordinal: index,
                        textual: false,
                        has_fill: true,
                        source: &IndexMap::new(),
                        fill_paths: &["themeVariables.edgeLabelBackground"],
                        stroke_paths: &[],
                        font_prefix: None,
                        fill_default: &binding.edge_background,
                        stroke_default: "none",
                        shadow: false,
                    },
                    work,
                )?;
                plan.edge_labels.insert(edge.id.clone(), label);
                plan.edge_backgrounds.insert(edge.id.clone(), background);
            }
            let sides: &[bool] = match edge.relationship_type {
                UsecaseRelationshipType::Generalization
                | UsecaseRelationshipType::Include
                | UsecaseRelationshipType::Extend => &[false],
                UsecaseRelationshipType::Association => match edge.arrow_type {
                    UsecaseArrowType::ReversedPoint
                    | UsecaseArrowType::ReversedCircle
                    | UsecaseArrowType::ReversedCross => &[true],
                    UsecaseArrowType::Line => &[],
                    _ => &[false],
                },
            };
            for start in sides {
                if source.contains_key("marker")
                    || source.contains_key(if *start { "marker-start" } else { "marker-end" })
                    || source.contains_key("all")
                {
                    continue;
                }
                let marker = plan.resolve_surface(
                    config,
                    Surface {
                        target: ThemeTarget::Marker,
                        ordinal: ordinal(ThemeTarget::Marker),
                        textual: false,
                        has_fill: !matches!(
                            edge.arrow_type,
                            UsecaseArrowType::Cross | UsecaseArrowType::ReversedCross
                        ),
                        source: &IndexMap::new(),
                        fill_paths: &["themeVariables.lineColor"],
                        stroke_paths: &["themeVariables.lineColor"],
                        font_prefix: None,
                        fill_default: if edge.relationship_type
                            == UsecaseRelationshipType::Generalization
                        {
                            &binding.main
                        } else {
                            &binding.line
                        },
                        stroke_default: &binding.line,
                        shadow: false,
                    },
                    work,
                )?;
                plan.markers.insert((edge.id.clone(), *start), marker);
            }
        }
        // Note connectors are real edge terminals; they share Edge styling without markers.
        for note in &model.notes {
            let edge = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Edge,
                    ordinal: ordinal(ThemeTarget::Edge),
                    textual: false,
                    has_fill: true,
                    source: &IndexMap::new(),
                    fill_paths: &[],
                    stroke_paths: &["themeVariables.lineColor"],
                    font_prefix: None,
                    fill_default: "none",
                    stroke_default: &binding.line,
                    shadow: false,
                },
                work,
            )?;
            plan.edges.insert(format!("{}-edge", note.id), edge);
        }
        if model
            .title
            .as_deref()
            .or(diagram_title)
            .is_some_and(|title| !title.is_empty())
        {
            plan.title = plan.resolve_surface(
                config,
                Surface {
                    target: ThemeTarget::Title,
                    ordinal: 1,
                    textual: true,
                    has_fill: true,
                    source: &IndexMap::new(),
                    fill_paths: NODE_TEXT,
                    stroke_paths: &[],
                    font_prefix: None,
                    fill_default: &binding.text,
                    stroke_default: "none",
                    shadow: false,
                },
                work,
            )?;
        }
        Ok(plan)
    }

    fn resolve_surface(
        &mut self,
        config: &PresentationConfig,
        surface: Surface<'_>,
        work: &OperationWorkMeter,
    ) -> crate::Result<UsecaseThemeStyle> {
        let theme = self.theme.as_ref().expect("themed surface");
        work.charge(
            theme
                .family_mechanism_routes()
                .len()
                .saturating_add(surface.source.len()),
        )?;
        let resolved = if surface.textual {
            theme.text_style_with_work_meter(
                surface.target,
                ThemeVariant::Default,
                Some(surface.ordinal),
                work,
            )?
        } else {
            theme.style_with_work_meter(
                surface.target,
                ThemeVariant::Default,
                Some(surface.ordinal),
                work,
            )?
        };
        let mut observations = Vec::new();
        let mut style = UsecaseThemeStyle::default();
        for route in theme.family_mechanism_routes().iter().copied() {
            let (property, key, facet, selector_supported) = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    facet,
                    selector,
                    ..
                } => {
                    let property = resolved_style_property_for_facet(facet);
                    if !resolved.winner_rule_properties().any(|(winner, origin)| {
                        winner == property && origin.rule_index() == rule_index
                    }) {
                        continue;
                    }
                    (
                        property,
                        theme.family_mechanism_key(route),
                        Some(facet),
                        matches!(
                            selector,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default)
                            }
                        ),
                    )
                }
                FamilyThemeMechanism::BaseTypography(property) if surface.textual => {
                    if resolved.typography_resolution().winner(property).is_some() {
                        continue;
                    }
                    (
                        ResolvedStyleProperty::Typography(property),
                        theme.family_mechanism_key(route),
                        None,
                        true,
                    )
                }
                _ => continue,
            };
            if source_owns(config, &surface, property) {
                continue;
            }
            let supported = route.disposition() == FamilyThemeDisposition::TypedAdapter
                && selector_supported
                && if property == ResolvedStyleProperty::Effect {
                    match theme.resolve_effect(surface.target, resolved.effect_resolution()) {
                        Some(ResolvedThemeEffect::ClearedByRule) if surface.shadow => {
                            style.clear_effect = true;
                            true
                        }
                        Some(ResolvedThemeEffect::Rule { graph }) if surface.shadow => {
                            style.shadow = graph.and_then(SvgShadowEffect::from_graph);
                            style.shadow.is_some()
                        }
                        _ => false,
                    }
                } else {
                    apply_property(theme, &mut style, &resolved, property, &surface)
                };
            observations.push(Observation {
                key,
                capability: supported
                    .then(|| capability(property, &resolved))
                    .flatten()
                    .filter(|_| !(property == ResolvedStyleProperty::Effect && style.clear_effect)),
                residual: (!supported).then(|| {
                    facet.map_or(
                        FamilyThemeResidualReason::UnsupportedTypography,
                        unsupported_residual_for_facet,
                    )
                }),
            });
        }
        if resolved.fill_resolution().winner().is_none()
            && !source_owns(config, &surface, ResolvedStyleProperty::Fill)
        {
            for target in
                std::iter::once(surface.target).chain(surface.textual.then_some(ThemeTarget::Text))
            {
                if theme.series_color(target, surface.ordinal).is_some() {
                    observations.push(Observation {
                        key: FamilyThemeMechanismKey::OrdinalPalette { target },
                        capability: None,
                        residual: Some(FamilyThemeResidualReason::UnsupportedOrdinalPalette),
                    });
                }
            }
        }
        let effect = theme
            .resolve_effect(surface.target, resolved.effect_resolution())
            .or_else(|| {
                surface
                    .textual
                    .then(|| theme.resolve_effect(ThemeTarget::Text, resolved.effect_resolution()))
                    .flatten()
            });
        if let Some(effect) = effect {
            match effect {
                ResolvedThemeEffect::Binding { binding, graph } => {
                    let key = FamilyThemeMechanismKey::EffectBinding {
                        target: binding.target(),
                        effect_id: binding.effect_id().to_owned(),
                    };
                    if !source_owns(config, &surface, ResolvedStyleProperty::Effect) {
                        let supported_route = theme.family_mechanism_routes().iter().any(|route| {
                            matches!(route.mechanism(), FamilyThemeMechanism::EffectBinding { target, .. } if target == surface.target)
                                && route.disposition() == FamilyThemeDisposition::TypedAdapter
                        });
                        let shadow = (surface.shadow && supported_route)
                            .then(|| graph.and_then(SvgShadowEffect::from_graph))
                            .flatten();
                        let supported = shadow.is_some();
                        style.shadow = shadow;
                        observations.push(Observation {
                            key,
                            capability: supported.then_some(ThemeCapability::Shadow),
                            residual: (!supported)
                                .then_some(FamilyThemeResidualReason::UnsupportedEffect),
                        });
                    }
                }
                ResolvedThemeEffect::Rule { .. } | ResolvedThemeEffect::ClearedByRule => {}
            }
        }
        if !observations.is_empty() {
            style.terminal = Some(self.terminals.len());
            self.terminals.push(observations);
        }
        Ok(style)
    }

    pub(crate) fn shell(&self, id: &str) -> Option<&UsecaseThemeStyle> {
        self.shells.get(id)
    }
    pub(crate) fn label(&self, id: &str) -> UsecaseThemeStyle {
        self.labels.get(id).cloned().unwrap_or_default()
    }
    pub(crate) fn stereotype(&self, id: &str) -> UsecaseThemeStyle {
        self.stereotypes.get(id).cloned().unwrap_or_default()
    }
    pub(crate) fn edge(&self, id: &str) -> Option<&UsecaseThemeStyle> {
        self.edges.get(id)
    }
    pub(crate) fn edge_label(&self, id: &str) -> UsecaseThemeStyle {
        self.edge_labels.get(id).cloned().unwrap_or_default()
    }
    pub(crate) fn edge_background(&self, id: &str) -> Option<&UsecaseThemeStyle> {
        self.edge_backgrounds.get(id)
    }
    pub(crate) fn marker(&self, id: &str, start: bool) -> Option<&UsecaseThemeStyle> {
        self.markers.get(&(id.to_owned(), start))
    }
    pub(crate) fn title(&self) -> &UsecaseThemeStyle {
        &self.title
    }

    pub(crate) fn retain_edge_terminals(
        &mut self,
        edges: &[super::super::UsecaseEdgePlan],
        work: &mut crate::layout_work::OperationLayoutWorkControl,
    ) -> crate::Result<()> {
        if self.theme.is_none() {
            return Ok(());
        }
        work.charge_adapter(
            edges
                .len()
                .saturating_add(self.edges.len())
                .saturating_add(self.edge_labels.len())
                .saturating_add(self.edge_backgrounds.len())
                .saturating_add(self.markers.len()),
        )?;
        let retained: std::collections::HashSet<_> = edges
            .iter()
            .map(|edge| edge.original_id.as_deref().unwrap_or(&edge.id))
            .collect();
        let terminals = &mut self.terminals;
        let mut retain = |id: &str, style: &UsecaseThemeStyle| {
            if retained.contains(id) {
                return true;
            }
            // Dagre can replace a parallel self-loop's entire projection. Only those
            // absent terminals leave the ledger; surviving terminals still require emission.
            if let Some(index) = style.terminal {
                terminals[index].clear();
            }
            false
        };
        self.edges.retain(|id, style| retain(id, style));
        self.edge_labels.retain(|id, style| retain(id, style));
        self.edge_backgrounds.retain(|id, style| retain(id, style));
        self.markers.retain(|(id, _), style| retain(id, style));
        Ok(())
    }

    pub(crate) fn has_background_fill(&self) -> bool {
        self.edge_backgrounds
            .values()
            .any(|style| style.fill.is_some())
    }
    pub(crate) fn begin_receipt(&self) -> UsecaseThemeReceipt {
        UsecaseThemeReceipt {
            emitted: RefCell::new(vec![false; self.terminals.len()]),
            ..Default::default()
        }
    }
    pub(crate) fn record_terminal(&self, receipt: UsecaseThemeReceipt) {
        let _ = self.receipt.set(UsecaseThemeSeal {
            emitted: receipt.emitted.into_inner(),
            effects: receipt.effects.into_inner(),
        });
    }
    pub(crate) fn expected_effect_applications(&self) -> usize {
        self.shells
            .values()
            .filter(|style| style.shadow.is_some())
            .count()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        let Some(theme) = &self.theme else { return };
        let mut outcomes = BTreeMap::<FamilyThemeMechanismKey, Outcome>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            outcomes
                .entry(theme.family_mechanism_key(route))
                .or_default();
        }
        for (index, observations) in self.terminals.iter().enumerate() {
            let emitted = self
                .receipt
                .get()
                .is_some_and(|receipt| receipt.emitted[index]);
            for observation in observations {
                let outcome = outcomes.entry(observation.key.clone()).or_default();
                if let Some(reason) = observation.residual {
                    outcome.residual.get_or_insert(reason);
                } else if emitted
                    && (observation.capability != Some(ThemeCapability::Shadow)
                        || self
                            .receipt
                            .get()
                            .is_some_and(|receipt| receipt.effects.contains(&index)))
                {
                    outcome.applied = true;
                    outcome.capabilities.extend(observation.capability);
                } else {
                    outcome
                        .residual
                        .get_or_insert(match observation.capability {
                            Some(ThemeCapability::Shadow) => {
                                FamilyThemeResidualReason::UnsupportedEffect
                            }
                            Some(ThemeCapability::Typography) => {
                                FamilyThemeResidualReason::UnsupportedTypography
                            }
                            _ => FamilyThemeResidualReason::UnsupportedPaint,
                        });
                }
            }
        }
        let mut result = FamilyThemeOutcomes::default();
        for (key, outcome) in outcomes {
            if let Some(reason) = outcome.residual {
                result.mark_residual(key, reason);
            } else if outcome.applied {
                result.mark_applied_with_capabilities(key, outcome.capabilities);
            } else {
                result.mark_not_applicable(key);
            }
        }
        evidence.merge_borrowed(&result);
    }
}

fn source_styles(
    model: &UsecaseDiagramModel,
    classes: &[String],
    inline: &[String],
) -> IndexMap<String, String> {
    // Normalize ownership queries only. Measurement and SVG keep the source spelling.
    super::super::measure::styles(model, classes, inline)
        .into_iter()
        .filter_map(|(key, value)| {
            let property =
                crate::mermaid_style::parse_style_declaration(&format!("{key}:{value}"))?
                    .property()
                    .to_owned();
            Some((property, value))
        })
        .collect()
}

fn bounded_body_source(source: &IndexMap<String, String>) -> bool {
    source.keys().all(|key| {
        crate::mermaid_style::is_label_style_key(key)
            || matches!(
                key.as_str(),
                "fill"
                    | "stroke"
                    | "stroke-width"
                    | "stroke-dasharray"
                    | "stroke-linecap"
                    | "stroke-linejoin"
                    | "opacity"
                    | "fill-opacity"
                    | "stroke-opacity"
            )
    })
}

fn source_owns(
    config: &PresentationConfig,
    surface: &Surface<'_>,
    property: ResolvedStyleProperty,
) -> bool {
    let owns = |key: &str| {
        (surface.source.contains_key(key)
            && (!surface.textual || crate::mermaid_style::is_label_style_key(key)))
            || (!surface.textual && surface.source.contains_key("all"))
    };
    let config_owns = |paths: &[&str]| {
        paths
            .iter()
            .any(|path| config.config_paint_overrides_typed_default(path))
    };
    match property {
        ResolvedStyleProperty::Fill => {
            !surface.has_fill
                || owns(if surface.textual { "color" } else { "fill" })
                || config_owns(surface.fill_paths)
        }
        ResolvedStyleProperty::Stroke => owns("stroke") || config_owns(surface.stroke_paths),
        ResolvedStyleProperty::StrokeWidth => owns("stroke-width"),
        ResolvedStyleProperty::StrokeDasharray => owns("stroke-dasharray"),
        ResolvedStyleProperty::StrokeLinecap => owns("stroke-linecap"),
        ResolvedStyleProperty::StrokeLinejoin => owns("stroke-linejoin"),
        ResolvedStyleProperty::Opacity => owns("opacity"),
        ResolvedStyleProperty::FillOpacity => owns("fill-opacity"),
        ResolvedStyleProperty::StrokeOpacity => owns("stroke-opacity"),
        ResolvedStyleProperty::Effect => owns("filter"),
        ResolvedStyleProperty::Typography(property) => {
            let (key, root, local) = match property {
                ThemeTypographyProperty::FontStack => ("font-family", "fontFamily", "FontFamily"),
                ThemeTypographyProperty::FontSize => ("font-size", "fontSize", "FontSize"),
                ThemeTypographyProperty::FontWeight => ("font-weight", "fontWeight", "FontWeight"),
                ThemeTypographyProperty::FontStyle => ("font-style", "fontStyle", "FontStyle"),
                _ => return false,
            };
            owns(key)
                || owns("font")
                // Role selectors and HTML labels consume their own size, even when the
                // generic theme variable is explicit. Ordinary SVG labels inherit it.
                || ((property != ThemeTypographyProperty::FontSize || surface.font_prefix.is_none())
                    && config.config_path_overrides_typed_default(&format!("themeVariables.{root}")))
                // Mermaid's generic size comes from themeVariables.fontSize, never fontSize.
                || (property != ThemeTypographyProperty::FontSize
                    && config.config_path_overrides_typed_default(root))
                || surface.font_prefix.is_some_and(|prefix| {
                    config.config_path_overrides_typed_default(&format!("usecase.{prefix}{local}"))
                })
        }
        _ => false,
    }
}

fn apply_property(
    theme: &ResolvedDiagramTheme,
    style: &mut UsecaseThemeStyle,
    resolved: &ResolvedThemeStyle,
    property: ResolvedStyleProperty,
    surface: &Surface<'_>,
) -> bool {
    match property {
        ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke => {
            if surface.textual && property == ResolvedStyleProperty::Stroke {
                return false;
            }
            let (paint, fallback, destination) = if property == ResolvedStyleProperty::Fill {
                (
                    resolved.fill_resolution().specified(),
                    surface.fill_default,
                    &mut style.fill,
                )
            } else {
                (
                    resolved.stroke_resolution().specified(),
                    surface.stroke_default,
                    &mut style.stroke,
                )
            };
            let targets = [ThemeTarget::Text, surface.target];
            let direct = if property == ResolvedStyleProperty::Fill {
                resolve_direct_static_fill(
                    theme,
                    resolved,
                    &targets,
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_stroke(
                    theme,
                    resolved,
                    &targets,
                    DirectStaticSelectorDomain::Default,
                )
            };
            *destination = match paint {
                Specified::Clear => Some(fallback.to_owned()),
                Specified::Value(CanvasPaint::Solid(_) | CanvasPaint::Transparent) => {
                    direct.map(|paint| paint.css().to_owned())
                }
                _ => return false,
            };
            if destination.is_none() {
                return false;
            }
        }
        ResolvedStyleProperty::StrokeWidth if !surface.textual => {
            style.stroke_width = resolved.stroke_width().map(f64::from);
        }
        ResolvedStyleProperty::StrokeDasharray if !surface.textual => {
            style.dasharray = resolved.stroke_dasharray().map(|values| {
                if values.is_empty() {
                    "none".to_owned()
                } else {
                    values
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            });
        }
        ResolvedStyleProperty::StrokeLinecap if !surface.textual => {
            style.linecap = resolved.stroke_linecap().map(|value| match value {
                crate::diagram_theme::StrokeLineCap::Round => "round",
                crate::diagram_theme::StrokeLineCap::Square => "square",
                crate::diagram_theme::StrokeLineCap::Butt => "butt",
            });
        }
        ResolvedStyleProperty::StrokeLinejoin if !surface.textual => {
            style.linejoin = resolved.stroke_linejoin().map(|value| match value {
                crate::diagram_theme::StrokeLineJoin::Round => "round",
                crate::diagram_theme::StrokeLineJoin::Bevel => "bevel",
                crate::diagram_theme::StrokeLineJoin::Miter => "miter",
            });
        }
        ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontStack)
            if surface.textual =>
        {
            style.font_family = Some(resolved.typography().font_stack().as_css());
        }
        ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize) if surface.textual => {
            style.font_size = Some(f64::from(resolved.typography().font_size_px()));
        }
        _ => return false,
    }
    true
}

fn capability(
    property: ResolvedStyleProperty,
    resolved: &ResolvedThemeStyle,
) -> Option<ThemeCapability> {
    match property {
        ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke => {
            let paint = if property == ResolvedStyleProperty::Fill {
                resolved.fill()
            } else {
                resolved.stroke()
            };
            match paint {
                Some(CanvasPaint::Solid(_)) => Some(ThemeCapability::SolidPaint),
                Some(CanvasPaint::Transparent) => Some(ThemeCapability::TransparentPaint),
                _ => None,
            }
        }
        ResolvedStyleProperty::StrokeDasharray => Some(ThemeCapability::DashStyling),
        ResolvedStyleProperty::StrokeWidth
        | ResolvedStyleProperty::StrokeLinecap
        | ResolvedStyleProperty::StrokeLinejoin => Some(ThemeCapability::BorderStyling),
        ResolvedStyleProperty::Typography(_) => Some(ThemeCapability::Typography),
        ResolvedStyleProperty::Effect => Some(ThemeCapability::Shadow),
        _ => None,
    }
}

const NODE_FILL: &[&str] = &[
    "themeVariables.usecaseBkg",
    "themeVariables.mainBkg",
    "themeVariables.primaryColor",
];
const NODE_STROKE: &[&str] = &[
    "themeVariables.usecaseBorder",
    "themeVariables.nodeBorder",
    "themeVariables.primaryColor",
];
const JSON_FILL: &[&str] = &["themeVariables.mainBkg", "themeVariables.primaryColor"];
const JSON_STROKE: &[&str] = &["themeVariables.nodeBorder", "themeVariables.primaryColor"];
const NODE_TEXT: &[&str] = &["themeVariables.primaryTextColor"];
const ACTOR_FILL: &[&str] = &[
    "themeVariables.usecaseActorBkg",
    "themeVariables.actorBkg",
    "themeVariables.mainBkg",
    "themeVariables.primaryColor",
];
const ACTOR_STROKE: &[&str] = &[
    "themeVariables.usecaseActorBorder",
    "themeVariables.actorBorder",
    "themeVariables.primaryColor",
];
const ACTOR_TEXT: &[&str] = &[
    "themeVariables.actorTextColor",
    "themeVariables.primaryTextColor",
];
