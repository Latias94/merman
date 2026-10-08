use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;

use super::theme::{XyChartTextRole, XyChartTypographyThemePlan, logical_axis, resolve_text_style};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRoute,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect,
    ResolvedThemeStyle, Specified, SvgShadowEffect, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::{XyChartDiagramLayout, XyChartDrawableElem};
use crate::resources::OperationWorkMeter;

/// The drawable position identifies the actual writer terminal, including renderer-created labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum XyChartPaintTerminalId {
    Text { drawable: usize, item: usize },
    Path { drawable: usize, item: usize },
    DataLabel { drawable: usize, item: usize },
}

#[derive(Debug)]
struct PaintTerminal {
    content: Box<str>,
    paint: Box<str>,
    dimension: f64,
    opacity: Option<f32>,
    font_weight: Option<String>,
    placement: Option<TextPlacement>,
}

#[derive(Debug)]
pub(crate) struct XyChartTextEffect {
    pub(crate) effect: Arc<SvgShadowEffect>,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

fn supports_text_effect(target: ThemeTarget) -> bool {
    matches!(
        target,
        ThemeTarget::Title | ThemeTarget::AxisTitle | ThemeTarget::AxisLabel | ThemeTarget::Legend
    )
}

#[derive(Debug, Clone)]
struct TextPlacement {
    transform: String,
    anchor: &'static str,
    baseline: &'static str,
}

impl TextPlacement {
    fn from_label(label: &crate::model::XyChartTextData) -> Self {
        let position = |value| {
            terminal_dimension(
                XyChartPaintTerminalId::Path {
                    drawable: 0,
                    item: 0,
                },
                value,
            )
        };
        Self {
            transform: format!(
                "translate({}, {}) rotate({})",
                position(label.x),
                position(label.y),
                position(label.rotation)
            ),
            anchor: match label.horizontal_pos.as_str() {
                "left" => "start",
                "right" => "end",
                _ => "middle",
            },
            baseline: if label.vertical_pos == "top" {
                "text-before-edge"
            } else {
                "middle"
            },
        }
    }
}

/// Role paint and typography retain author order and property-local source ownership.
#[derive(Debug, Default)]
pub(crate) struct XyChartPaintPlan {
    terminals: Arc<BTreeMap<XyChartPaintTerminalId, PaintTerminal>>,
    data_label_color: String,
    tick_opacity: Option<f32>,
    text_effects: BTreeMap<XyChartPaintTerminalId, XyChartTextEffect>,
    pending_bindings: BTreeSet<FamilyThemeMechanismKey>,
    pending: BTreeMap<(ThemeTarget, usize), BTreeSet<ThemeCapability>>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<()>,
}

#[derive(Default)]
struct RuleObservation {
    pending: BTreeSet<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

impl XyChartPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        layout: &mut XyChartDiagramLayout,
        typography: &XyChartTypographyThemePlan,
        measurer: &dyn crate::text::TextMeasurer,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(theme),
            data_label_color: crate::config::config_string(
                config.as_value(),
                &["themeVariables", "xyChart", "dataLabelColor"],
            )
            .or_else(|| {
                crate::config::config_string(
                    config.as_value(),
                    &["themeVariables", "primaryTextColor"],
                )
            })
            .unwrap_or_else(|| "black".to_owned()),
            ..Self::default()
        };
        let Some(theme) = theme else {
            return Ok(plan);
        };
        let mut accounting = PaintAccounting::new(theme, work_meter);
        if accounting.routes.is_empty() {
            return Ok(plan);
        }
        let text_styles = [
            ThemeTarget::Text,
            ThemeTarget::Title,
            ThemeTarget::AxisTitle,
            ThemeTarget::AxisLabel,
            ThemeTarget::Legend,
        ]
        .into_iter()
        .map(|target| resolve_text_style(theme, target, work_meter).map(|style| (target, style)))
        .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        // Axis geometry never inherits generic Text paint.
        let line_style = theme.style_with_work_meter(
            ThemeTarget::Axis,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let tick_style = if accounting.routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::AxisTick,
                    ..
                }
            )
        }) {
            let mut style = line_style.clone();
            style.merge_from(&theme.style_with_work_meter(
                ThemeTarget::AxisTick,
                ThemeVariant::Default,
                None,
                work_meter,
            )?);
            Some(style)
        } else {
            None
        };
        if let Some(style) = &tick_style
            && style
                .opacity_resolution()
                .winner()
                .is_some_and(|origin| origin.target() == ThemeTarget::AxisTick)
        {
            plan.tick_opacity = style.opacity_resolution().value().copied();
        }
        let mut effects = BTreeMap::new();
        for (&target, style) in &text_styles {
            if !supports_text_effect(target) {
                continue;
            }
            if let Some((_, origin)) = style
                .winner_rule_properties()
                .find(|(property, _)| *property == ResolvedStyleProperty::Effect)
            {
                let supported = accounting.routes.iter().any(|route| {
                    route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(route.mechanism(), FamilyThemeMechanism::RuleFacet { rule_index, facet: crate::diagram_theme::FamilyThemeRuleFacet::Effect, .. } if rule_index == origin.rule_index())
                });
                if !supported {
                    continue;
                }
            }
            let graph = match theme.resolve_effect(target, style.effect_resolution()) {
                Some(
                    ResolvedThemeEffect::Rule { graph }
                    | ResolvedThemeEffect::Binding { graph, .. },
                ) => graph,
                _ => None,
            };
            if let Some(effect) = graph.and_then(SvgShadowEffect::from_graph) {
                effects.insert(target, Arc::new(effect));
            }
        }
        for (drawable, shape) in layout.drawables.iter_mut().enumerate() {
            match shape {
                XyChartDrawableElem::Text { group_texts, data } => {
                    let Some((target, channel)) =
                        text_channel(group_texts, &layout.chart_orientation)
                    else {
                        continue;
                    };
                    let style = &text_styles[&target];
                    let source_owned = source_owns(config, channel);
                    let paint = (!source_owned)
                        .then(|| {
                            resolve_direct_static_fill(
                                theme,
                                style,
                                &[
                                    ThemeTarget::Text,
                                    ThemeTarget::Title,
                                    ThemeTarget::Axis,
                                    ThemeTarget::AxisTitle,
                                    ThemeTarget::AxisLabel,
                                    ThemeTarget::Legend,
                                ],
                                DirectStaticSelectorDomain::Default,
                            )
                        })
                        .flatten();
                    work_meter.charge(data.len())?;
                    for (item, label) in data.iter_mut().enumerate() {
                        if let Some(paint) = &paint {
                            label.fill = paint.css().to_owned();
                        }
                        if visible_dimension(label.font_size) && !label.text.trim().is_empty() {
                            let effect = effects.get(&target);
                            if let Some(effect) = effect {
                                work_meter.charge(label.text.len())?;
                                let dimension = super::max_text_dimension(
                                    std::slice::from_ref(&label.text),
                                    label.font_size,
                                    typography.font_family_css(),
                                    XyChartTextRole::from_groups(
                                        group_texts,
                                        &layout.chart_orientation,
                                    )
                                    .and_then(|role| typography.font_weight(role)),
                                    measurer,
                                );
                                plan.text_effects.insert(
                                    XyChartPaintTerminalId::Text { drawable, item },
                                    XyChartTextEffect {
                                        effect: effect.clone(),
                                        width: dimension.width,
                                        height: dimension.height,
                                    },
                                );
                            }
                            accounting.observe(
                                target,
                                channel,
                                source_owned,
                                style,
                                paint.as_ref(),
                                effect.is_some(),
                                XyChartTextRole::from_groups(
                                    group_texts,
                                    &layout.chart_orientation,
                                )
                                .map(|role| (typography, role)),
                            )?;
                        }
                    }
                }
                XyChartDrawableElem::Path { group_texts, data } => {
                    let Some((target, channel)) =
                        line_channel(group_texts, &layout.chart_orientation)
                    else {
                        continue;
                    };
                    let style = if target == ThemeTarget::AxisTick {
                        tick_style.as_ref().unwrap_or(&line_style)
                    } else {
                        &line_style
                    };
                    let source_owned = source_owns(config, channel);
                    let paint = (!source_owned)
                        .then(|| {
                            if matches!(
                                style.stroke_resolution().specified(),
                                Specified::Unspecified
                            ) {
                                resolve_direct_static_fill(
                                    theme,
                                    style,
                                    &[ThemeTarget::Axis, ThemeTarget::AxisTick],
                                    DirectStaticSelectorDomain::Default,
                                )
                            } else {
                                resolve_direct_static_stroke(
                                    theme,
                                    style,
                                    &[ThemeTarget::Axis, ThemeTarget::AxisTick],
                                    DirectStaticSelectorDomain::Default,
                                )
                            }
                        })
                        .flatten();
                    work_meter.charge(data.len())?;
                    for path in data {
                        if let Some(paint) = &paint {
                            path.stroke_fill = paint.css().to_owned();
                        }
                        if visible_dimension(path.stroke_width) && visible_axis_segment(&path.path)
                        {
                            accounting.observe(
                                target,
                                channel,
                                source_owned,
                                style,
                                paint.as_ref(),
                                false,
                                None,
                            )?;
                        }
                    }
                }
                XyChartDrawableElem::Rect { data, .. } => {
                    if !layout.show_data_label {
                        continue;
                    }
                    let channel = "dataLabelColor";
                    let style = &text_styles[&ThemeTarget::Text];
                    let source_owned = source_owns(config, channel);
                    let paint = (!source_owned)
                        .then(|| {
                            resolve_direct_static_fill(
                                theme,
                                style,
                                &[ThemeTarget::Text],
                                DirectStaticSelectorDomain::Default,
                            )
                        })
                        .flatten();
                    if let Some(paint) = &paint {
                        plan.data_label_color = paint.css().to_owned();
                    }
                    work_meter.charge(data.len())?;
                    let labels = super::rect_data_labels(
                        data,
                        &layout.label_data,
                        &layout.chart_orientation,
                    );
                    for label in labels.items {
                        if visible_dimension(labels.font_size) && !label.label.trim().is_empty() {
                            accounting.observe(
                                ThemeTarget::Text,
                                channel,
                                source_owned,
                                style,
                                paint.as_ref(),
                                false,
                                None,
                            )?;
                        }
                    }
                }
            }
        }
        accounting.finish(&mut plan);
        if plan.needs_terminal_receipt() {
            plan.capture_terminals(layout, typography);
        }
        Ok(plan)
    }

    fn capture_terminals(
        &mut self,
        layout: &XyChartDiagramLayout,
        typography: &XyChartTypographyThemePlan,
    ) {
        // Source-owned, absent, and unsupported requests need no writer receipt payload.
        let mut terminals = BTreeMap::new();
        for (drawable, shape) in layout.drawables.iter().enumerate() {
            match shape {
                XyChartDrawableElem::Text { group_texts, data }
                    if text_channel(group_texts, &layout.chart_orientation).is_some() =>
                {
                    for (item, label) in data.iter().enumerate() {
                        terminals.insert(
                            XyChartPaintTerminalId::Text { drawable, item },
                            PaintTerminal {
                                content: label.text.clone().into_boxed_str(),
                                paint: label.fill.clone().into_boxed_str(),
                                dimension: label.font_size,
                                opacity: None,
                                placement: self
                                    .text_effects
                                    .contains_key(&XyChartPaintTerminalId::Text { drawable, item })
                                    .then(|| TextPlacement::from_label(label)),
                                font_weight: XyChartTextRole::from_groups(
                                    group_texts,
                                    &layout.chart_orientation,
                                )
                                .and_then(|role| typography.font_weight(role))
                                .map(str::to_owned),
                            },
                        );
                    }
                }
                XyChartDrawableElem::Path { group_texts, data }
                    if line_channel(group_texts, &layout.chart_orientation).is_some() =>
                {
                    for (item, path) in data.iter().enumerate() {
                        terminals.insert(
                            XyChartPaintTerminalId::Path { drawable, item },
                            PaintTerminal {
                                content: path.path.clone().into_boxed_str(),
                                paint: path.stroke_fill.clone().into_boxed_str(),
                                dimension: path.stroke_width,
                                opacity: self.path_opacity(group_texts, &layout.chart_orientation),
                                font_weight: None,
                                placement: None,
                            },
                        );
                    }
                }
                XyChartDrawableElem::Rect { data, .. } if layout.show_data_label => {
                    let labels = super::rect_data_labels(
                        data,
                        &layout.label_data,
                        &layout.chart_orientation,
                    );
                    for label in labels.items {
                        terminals.insert(
                            XyChartPaintTerminalId::DataLabel {
                                drawable,
                                item: label.item,
                            },
                            PaintTerminal {
                                content: label.label.into(),
                                paint: self.data_label_color.clone().into_boxed_str(),
                                dimension: labels.font_size,
                                opacity: None,
                                font_weight: None,
                                placement: None,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        self.terminals = Arc::new(terminals);
    }

    fn needs_terminal_receipt(&self) -> bool {
        !self.pending.is_empty()
            || !self.pending_bindings.is_empty()
            || !self.text_effects.is_empty()
    }

    pub(crate) fn text_effect(&self, id: XyChartPaintTerminalId) -> Option<&XyChartTextEffect> {
        self.text_effects.get(&id)
    }

    pub(crate) fn expected_effect_applications(&self) -> usize {
        self.text_effects.len()
    }

    pub(crate) fn filter_id(&self, diagram_id: &str, id: XyChartPaintTerminalId) -> Option<String> {
        let XyChartPaintTerminalId::Text { drawable, item } = id else {
            return None;
        };
        let effect = self.text_effect(id)?;
        Some(format!(
            "{diagram_id}-xychart-text-{drawable}-{item}-theme-effect-{}",
            effect.effect.id()
        ))
    }

    pub(crate) fn path_opacity(&self, groups: &[String], orientation: &str) -> Option<f32> {
        let opacity = self.tick_opacity?;
        if matches!(
            line_channel(groups, orientation),
            Some((ThemeTarget::AxisTick, _))
        ) {
            Some(opacity)
        } else {
            None
        }
    }

    pub(crate) fn data_label_color(&self) -> &str {
        &self.data_label_color
    }

    pub(crate) fn terminal_id(&self, id: XyChartPaintTerminalId) -> Option<XyChartPaintTerminalId> {
        self.terminals.contains_key(&id).then_some(id)
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        escape_xml: impl Fn(&str) -> String,
        diagram_id: &str,
    ) -> Option<XyChartPaintReceipt> {
        self.needs_terminal_receipt().then(|| XyChartPaintReceipt {
            owner: Arc::clone(&self.terminals),
            expected: self
                .terminals
                .iter()
                .map(|(&id, terminal)| {
                    (
                        id,
                        ExpectedPaintTerminal {
                            content: escape_xml(&terminal.content),
                            paint: escape_xml(&terminal.paint),
                            dimension: terminal_dimension(id, terminal.dimension),
                            opacity: terminal.opacity.map(|value| value.to_string()),
                            font_weight: terminal.font_weight.clone(),
                            filter: self
                                .filter_id(diagram_id, id)
                                .map(|id| escape_xml(&format!("url(#{id})"))),
                            placement: terminal.placement.clone(),
                        },
                    )
                })
                .collect(),
            seen: BTreeSet::new(),
            valid: true,
        })
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartPaintReceipt) -> bool {
        Arc::ptr_eq(&self.terminals, &receipt.owner)
            && receipt.valid
            && receipt.seen.len() == self.terminals.len()
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some() {
            for key in &self.pending_bindings {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                );
            }
            for (&(target, index), capabilities) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule { index, target },
                    capabilities.iter().copied(),
                );
            }
        }
        evidence
    }
}

struct PaintAccounting<'a> {
    theme: &'a ResolvedDiagramTheme,
    meter: &'a OperationWorkMeter,
    routes: Vec<FamilyThemeRoute>,
    observations: BTreeMap<(ThemeTarget, usize), RuleObservation>,
    palette_targets: BTreeSet<ThemeTarget>,
    effect_targets: BTreeSet<ThemeTarget>,
    consumed_effect_targets: BTreeSet<ThemeTarget>,
    static_channels: BTreeSet<(&'static str, bool)>,
    has_ordinal: bool,
    text_ordinal: usize,
    role_ordinals: BTreeMap<(ThemeTarget, bool), usize>,
}

impl<'a> PaintAccounting<'a> {
    fn new(theme: &'a ResolvedDiagramTheme, meter: &'a OperationWorkMeter) -> Self {
        let routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text
                            | ThemeTarget::Title
                            | ThemeTarget::Axis
                            | ThemeTarget::AxisTick
                            | ThemeTarget::AxisTitle
                            | ThemeTarget::AxisLabel
                            | ThemeTarget::Legend,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text
                            | ThemeTarget::Title
                            | ThemeTarget::Axis
                            | ThemeTarget::AxisTick
                            | ThemeTarget::AxisTitle
                            | ThemeTarget::AxisLabel
                            | ThemeTarget::Legend
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text
                            | ThemeTarget::Title
                            | ThemeTarget::Axis
                            | ThemeTarget::AxisTick
                            | ThemeTarget::AxisTitle
                            | ThemeTarget::AxisLabel
                            | ThemeTarget::Legend,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        let observations = routes
            .iter()
            .filter_map(|route| {
                if let FamilyThemeMechanism::RuleFacet {
                    target, rule_index, ..
                } = route.mechanism()
                {
                    Some(((target, rule_index), RuleObservation::default()))
                } else {
                    None
                }
            })
            .collect();
        let has_ordinal = routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    selector: FamilyThemeSelectorShape::Ordinal { .. },
                    ..
                }
            )
        });
        Self {
            theme,
            meter,
            routes,
            observations,
            has_ordinal,
            palette_targets: BTreeSet::new(),
            effect_targets: BTreeSet::new(),
            consumed_effect_targets: BTreeSet::new(),
            static_channels: BTreeSet::new(),
            text_ordinal: 0,
            role_ordinals: BTreeMap::new(),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The family terminal receipt reconciles emitted geometry, paint, and source ownership."
    )]
    fn observe(
        &mut self,
        target: ThemeTarget,
        channel: &'static str,
        source_owned: bool,
        static_style: &ResolvedThemeStyle,
        paint: Option<&DirectStaticPaint>,
        effect_consumed: bool,
        typography: Option<(&XyChartTypographyThemePlan, XyChartTextRole)>,
    ) -> crate::Result<()> {
        // Axis text uses its concrete roles; only geometry retains the Axis target here.
        let line = matches!(target, ThemeTarget::Axis | ThemeTarget::AxisTick);
        if !line {
            self.text_ordinal += 1;
        }
        let axis_ordinal = if matches!(
            target,
            ThemeTarget::AxisTick | ThemeTarget::AxisTitle | ThemeTarget::AxisLabel
        ) {
            let ordinal = self
                .role_ordinals
                .entry((ThemeTarget::Axis, line))
                .or_default();
            *ordinal += 1;
            Some(*ordinal)
        } else {
            None
        };
        let ordinal = self.role_ordinals.entry((target, line)).or_default();
        *ordinal += 1;
        let ordinal = *ordinal;
        if !self.has_ordinal && !self.static_channels.insert((channel, line)) {
            return Ok(());
        }
        let mut dynamic;
        let style = if self.has_ordinal {
            if line {
                dynamic = self.theme.style_with_work_meter(
                    ThemeTarget::Axis,
                    ThemeVariant::Default,
                    Some(axis_ordinal.unwrap_or(ordinal)),
                    self.meter,
                )?;
                if target == ThemeTarget::AxisTick {
                    dynamic.merge_from(&self.theme.style_with_work_meter(
                        target,
                        ThemeVariant::Default,
                        Some(ordinal),
                        self.meter,
                    )?);
                }
            } else {
                dynamic = self.theme.style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    Some(self.text_ordinal),
                    self.meter,
                )?;
                if let Some(axis_ordinal) = axis_ordinal {
                    dynamic.merge_from(&self.theme.style_with_work_meter(
                        ThemeTarget::Axis,
                        ThemeVariant::Default,
                        Some(axis_ordinal),
                        self.meter,
                    )?);
                }
                if target != ThemeTarget::Text {
                    let role = self.theme.style_with_work_meter(
                        target,
                        ThemeVariant::Default,
                        Some(ordinal),
                        self.meter,
                    )?;
                    dynamic.merge_from(&role);
                }
            }
            &dynamic
        } else {
            static_style
        };
        let property = if line
            && !matches!(
                style.stroke_resolution().specified(),
                Specified::Unspecified
            ) {
            ResolvedStyleProperty::Stroke
        } else {
            ResolvedStyleProperty::Fill
        };
        let targets: &[ThemeTarget] = if target == ThemeTarget::AxisTick {
            &[ThemeTarget::Axis, ThemeTarget::AxisTick]
        } else if line {
            &[ThemeTarget::Axis]
        } else if target == ThemeTarget::Text {
            &[ThemeTarget::Text]
        } else if matches!(target, ThemeTarget::AxisTitle | ThemeTarget::AxisLabel) {
            &[ThemeTarget::Text, ThemeTarget::Axis, target]
        } else {
            &[ThemeTarget::Text, target]
        };
        if !source_owned
            && property == ResolvedStyleProperty::Fill
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
        {
            self.palette_targets.extend(targets.iter().copied());
        }
        if matches!(
            style.effect_resolution().specified(),
            Specified::Unspecified
        ) {
            self.effect_targets.extend(targets.iter().copied());
            if effect_consumed {
                self.consumed_effect_targets.insert(target);
            }
        }
        self.meter.charge(self.routes.len())?;
        for route in &self.routes {
            let FamilyThemeMechanism::RuleFacet {
                target: route_target,
                rule_index,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !targets.contains(&route_target) {
                continue;
            }
            let candidate = resolved_style_property_for_facet(facet);
            // Typography has no terminal on an axis path. Its real text winners are
            // reconciled separately, including any overriding role-local rules.
            if line
                && route_target == ThemeTarget::Axis
                && matches!(candidate, ResolvedStyleProperty::Typography(_))
            {
                continue;
            }
            // Axis stroke owns geometry; it never paints the text terminal itself.
            if !line
                && route_target == ThemeTarget::Axis
                && candidate == ResolvedStyleProperty::Stroke
            {
                continue;
            }
            if (line
                && matches!(
                    candidate,
                    ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                )
                && candidate != property)
                || (source_owned && candidate == property)
                || (candidate
                    == ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize)
                    && typography.is_some_and(|(plan, role)| plan.source_owns_size(role)))
            {
                continue;
            }
            if !style
                .winner_rule_properties()
                .any(|(winner, origin)| winner == candidate && origin.rule_index() == rule_index)
            {
                continue;
            }
            let observation = self
                .observations
                .get_mut(&(route_target, rule_index))
                .expect("registered XYChart rule");
            match route.disposition() {
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
                FamilyThemeDisposition::TypedAdapter
                    if line
                        && candidate == ResolvedStyleProperty::Opacity
                        && style.opacity_resolution() == static_style.opacity_resolution() =>
                {
                    observation.pending.insert(ThemeCapability::Opacity);
                }
                FamilyThemeDisposition::TypedAdapter
                    if line
                        && candidate == property
                        && ((candidate == ResolvedStyleProperty::Stroke
                            && matches!(
                                style.stroke_resolution().specified(),
                                Specified::Clear
                            ))
                            || (candidate == ResolvedStyleProperty::Fill
                                && matches!(
                                    style.fill_resolution().specified(),
                                    Specified::Clear
                                ))) =>
                {
                    observation.pending.insert(ThemeCapability::SemanticRules);
                }
                FamilyThemeDisposition::TypedAdapter
                    if candidate == ResolvedStyleProperty::Effect
                        && style.effect_resolution() == static_style.effect_resolution()
                        && (effect_consumed
                            || matches!(
                                style.effect_resolution().specified(),
                                Specified::Clear
                            )) =>
                {
                    observation.pending.insert(ThemeCapability::SemanticRules);
                    if effect_consumed {
                        observation
                            .pending
                            .extend([ThemeCapability::Shadow, ThemeCapability::SvgFilter]);
                    }
                }
                FamilyThemeDisposition::TypedAdapter
                    if candidate == ResolvedStyleProperty::Effect =>
                {
                    observation
                        .residual
                        .get_or_insert(FamilyThemeResidualReason::UnsupportedEffect);
                }
                FamilyThemeDisposition::TypedAdapter
                    if candidate == property
                        && paint.is_some_and(|paint| paint.rule_index() == rule_index) =>
                {
                    observation
                        .pending
                        .insert(paint.expect("matched terminal paint").capability());
                }
                FamilyThemeDisposition::TypedAdapter
                    if !line
                        && candidate == ResolvedStyleProperty::Fill
                        && matches!(style.fill_resolution().specified(), Specified::Clear) =>
                {
                    // The layout keeps its configured fill when the winning rule clears paint.
                    // Its unchanged terminal still needs to be observed by the writer.
                    observation.pending.insert(ThemeCapability::SemanticRules);
                }
                FamilyThemeDisposition::TypedAdapter
                    if matches!(candidate, ResolvedStyleProperty::Typography(property)
                        if typography.is_some_and(|(plan, role)| plan.consumes(role, property, rule_index))) =>
                {
                    observation.pending.insert(ThemeCapability::Typography);
                }
                _ => observation.incomplete = true,
            }
        }
        Ok(())
    }

    fn finish(self, plan: &mut XyChartPaintPlan) {
        for route in &self.routes {
            let (applies, reason) = match route.mechanism() {
                FamilyThemeMechanism::OrdinalPalette { target } => (
                    self.palette_targets.contains(&target),
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                ),
                FamilyThemeMechanism::EffectBinding { target, .. } => (
                    self.effect_targets.contains(&target),
                    FamilyThemeResidualReason::UnsupportedEffect,
                ),
                _ => continue,
            };
            let key = self.theme.family_mechanism_key(*route);
            if applies {
                if let FamilyThemeMechanism::EffectBinding { target, .. } = route.mechanism()
                    && self.consumed_effect_targets.contains(&target)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
                {
                    plan.pending_bindings.insert(key);
                } else {
                    plan.evidence.mark_residual(key, reason);
                }
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        for ((target, index), observation) in self.observations {
            let key = FamilyThemeMechanismKey::Rule { index, target };
            if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A consumed facet cannot certify another winning sibling facet.
            } else if !observation.pending.is_empty() {
                plan.pending.insert((target, index), observation.pending);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
    }
}

// Use the same canonical number representation as the SVG writer, including its
// distinct handling of non-finite XY coordinates/widths and ordinary text sizes.
fn terminal_dimension(id: XyChartPaintTerminalId, value: f64) -> String {
    let number = if !matches!(id, XyChartPaintTerminalId::Text { .. }) && !value.is_finite() {
        "NaN".to_owned()
    } else {
        crate::number_format::canonical_number(value).to_string()
    };
    if matches!(id, XyChartPaintTerminalId::DataLabel { .. }) {
        format!("{number}px")
    } else {
        number
    }
}

// XY Chart emits each axis line and tick as one absolute straight segment. Check
// the actual endpoints: tiny nonzero tick lengths can disappear during layout arithmetic.
fn visible_axis_segment(path: &str) -> bool {
    use svgtypes::PathSegment;

    let mut segments = svgtypes::PathParser::from(path);
    let Some(Ok(PathSegment::MoveTo {
        abs: true,
        x: x0,
        y: y0,
    })) = segments.next()
    else {
        return false;
    };
    let Some(Ok(PathSegment::LineTo {
        abs: true,
        x: x1,
        y: y1,
    })) = segments.next()
    else {
        return false;
    };
    segments.next().is_none()
        && [x0, y0, x1, y1].into_iter().all(f64::is_finite)
        && (x0 != x1 || y0 != y1)
}

fn visible_dimension(value: f64) -> bool {
    value.is_finite() && crate::number_format::canonicalize_number(value) > 0.0
}

fn source_owns(config: &MermaidConfig, channel: &str) -> bool {
    merman_core::__private::config_path_overrides_typed_default(
        config,
        &format!("themeVariables.xyChart.{channel}"),
    )
}

fn text_channel(groups: &[String], orientation: &str) -> Option<(ThemeTarget, &'static str)> {
    let role = XyChartTextRole::from_groups(groups, orientation)?;
    let channel = match role {
        XyChartTextRole::Title => "titleColor",
        XyChartTextRole::XAxisTitle => "xAxisTitleColor",
        XyChartTextRole::YAxisTitle => "yAxisTitleColor",
        XyChartTextRole::XAxisLabel => "xAxisLabelColor",
        XyChartTextRole::YAxisLabel => "yAxisLabelColor",
        XyChartTextRole::Legend => "legendTextColor",
    };
    Some((role.target(), channel))
}

fn line_channel(groups: &[String], orientation: &str) -> Option<(ThemeTarget, &'static str)> {
    if groups.len() != 2 {
        return None;
    }
    let x = logical_axis(&groups[0], orientation)?;
    match (x, groups[1].as_str()) {
        (true, "axis-line" | "axisl-line") => Some((ThemeTarget::Axis, "xAxisLineColor")),
        (true, "ticks") => Some((ThemeTarget::AxisTick, "xAxisTickColor")),
        (false, "axis-line" | "axisl-line") => Some((ThemeTarget::Axis, "yAxisLineColor")),
        (false, "ticks") => Some((ThemeTarget::AxisTick, "yAxisTickColor")),
        _ => None,
    }
}

#[derive(Debug)]
struct ExpectedPaintTerminal {
    content: String,
    paint: String,
    dimension: String,
    opacity: Option<String>,
    font_weight: Option<String>,
    filter: Option<String>,
    placement: Option<TextPlacement>,
}

#[derive(Debug)]
pub(crate) struct XyChartPaintReceipt {
    owner: Arc<BTreeMap<XyChartPaintTerminalId, PaintTerminal>>,
    expected: BTreeMap<XyChartPaintTerminalId, ExpectedPaintTerminal>,
    seen: BTreeSet<XyChartPaintTerminalId>,
    valid: bool,
}

impl XyChartPaintReceipt {
    pub(crate) fn record<'a>(
        &mut self,
        id: XyChartPaintTerminalId,
        tag: &str,
        content: Option<&str>,
        attribute: impl Fn(&str) -> Option<&'a str>,
    ) {
        let path = matches!(id, XyChartPaintTerminalId::Path { .. });
        self.valid &= tag == if path { "path" } else { "text" }
            && self.seen.insert(id)
            && self.expected.get(&id).is_some_and(|expected| {
                content == Some(expected.content.as_str())
                    && attribute(if path { "stroke" } else { "fill" })
                        == Some(expected.paint.as_str())
                    && attribute(if path { "stroke-width" } else { "font-size" })
                        == Some(expected.dimension.as_str())
                    && attribute("opacity") == expected.opacity.as_deref()
                    && attribute("font-weight") == expected.font_weight.as_deref()
                    && attribute("filter") == expected.filter.as_deref()
                    && expected.placement.as_ref().is_none_or(|placement| {
                        attribute("transform") == Some(placement.transform.as_str())
                            && attribute("text-anchor") == Some(placement.anchor)
                            && attribute("dominant-baseline") == Some(placement.baseline)
                    })
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };

    #[test]
    fn source_owned_text_retains_no_receipt_payload_at_any_scale() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let typography = XyChartTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive()),
        )
        .unwrap();
        let source_config = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": {"xyChart": {"titleColor": "black"}}
            })))
            .parse_metadata_sync("xychart\nx-axis [A]\ny-axis 0 --> 10\nbar [4]\n")
            .unwrap()
            .effective_config;
        for count in [1, 64, 1024] {
            let mut layout: XyChartDiagramLayout = serde_json::from_value(serde_json::json!({
                "width": 700.0, "height": 500.0, "chartOrientation": "vertical",
                "showDataLabel": false, "showDataLabelOutsideBar": false,
                "labelData": [], "backgroundColor": "white", "drawables": []
            }))
            .unwrap();
            let label = crate::model::XyChartTextData {
                text: "A & B ".repeat(1024),
                x: 0.0,
                y: 0.0,
                fill: "black".into(),
                font_size: 20.0,
                rotation: 0.0,
                vertical_pos: "middle".into(),
                horizontal_pos: "center".into(),
            };
            layout.drawables.push(XyChartDrawableElem::Text {
                group_texts: vec!["chart-title".into()],
                data: vec![label; count],
            });
            let plan = XyChartPaintPlan::resolve(
                Some(&theme),
                &source_config,
                &mut layout,
                &typography,
                &crate::text::DeterministicTextMeasurer::default(),
                &OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive()),
            )
            .unwrap();
            assert!(plan.terminals.is_empty(), "source-owned count={count}");
            assert!(
                plan.begin_terminal_receipt(|_| panic!("no payload to escape"), "test")
                    .is_none()
            );
            assert!(plan.finish_evidence().applied().is_empty());
            assert_eq!(
                plan.finish_evidence().not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Text
                }]
            );
            let XyChartDrawableElem::Text { data, .. } = &layout.drawables[0] else {
                panic!()
            };
            assert!(data.iter().all(|label| label.fill == "black"));

            let pending = XyChartPaintPlan::resolve(
                Some(&theme),
                &MermaidConfig::default(),
                &mut layout,
                &typography,
                &crate::text::DeterministicTextMeasurer::default(),
                &OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive()),
            )
            .unwrap();
            assert_eq!(pending.terminals.len(), count);
            let receipt = pending
                .begin_terminal_receipt(str::to_owned, "test")
                .unwrap();
            assert_eq!(receipt.expected.len(), count);
            assert!(
                pending
                    .terminals
                    .values()
                    .all(|terminal| terminal.paint.as_ref() == "#123456")
            );
        }
    }

    #[test]
    fn zero_sized_rectangle_labels_are_checked_but_cannot_certify_text_paint() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let typography = XyChartTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive()),
        )
        .unwrap();
        let mut layout: XyChartDiagramLayout = serde_json::from_value(serde_json::json!({
            "width": 700.0, "height": 500.0, "chartOrientation": "vertical",
            "showDataLabel": true, "showDataLabelOutsideBar": false,
            "labelData": ["4"], "backgroundColor": "white", "drawables": []
        }))
        .unwrap();
        layout.drawables.push(XyChartDrawableElem::Rect {
            group_texts: vec!["legend".into(), "markers".into()],
            data: vec![crate::model::XyChartRectData {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
                fill: "red".into(),
                stroke_fill: "red".into(),
                stroke_width: 0.0,
            }],
        });
        let plan = XyChartPaintPlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &mut layout,
            &typography,
            &crate::text::DeterministicTextMeasurer::default(),
            &OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive()),
        )
        .unwrap();
        assert!(
            plan.terminal_id(XyChartPaintTerminalId::DataLabel {
                drawable: 0,
                item: 0
            })
            .is_none()
        );
        assert_eq!(plan.data_label_color(), "#123456");
        assert!(plan.begin_terminal_receipt(str::to_owned, "test").is_none());
        assert!(plan.finish_evidence().applied().is_empty());
        assert_eq!(
            plan.finish_evidence().not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text
            }]
        );
    }
}
