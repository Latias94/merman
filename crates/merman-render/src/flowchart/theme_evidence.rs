use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeRuleFacet, MatchedThemeRules, ResolvedDiagramTheme, ResolvedProperty,
    ResolvedStyleProperty, ResolvedThemeEffect, SourceStyleResidual, Specified, SvgShadowEffect,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason,
    unsupported_residual_for_facet as unsupported_reason_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, PartialEq)]
enum FlowchartPaintOutcome {
    Candidate {
        rule_index: usize,
        value: String,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

impl FlowchartPaintOutcome {
    fn value(&self) -> Option<&str> {
        match self {
            Self::Candidate { value, .. } => Some(value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug, PartialEq)]
enum FlowchartScalarOutcome {
    Candidate {
        rule_index: usize,
        value: f32,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

#[derive(Debug)]
enum FlowchartPaddingOutcome {
    Candidate {
        rule_index: usize,
        value: super::FlowchartEdgeLabelPadding,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

impl FlowchartPaddingOutcome {
    const fn value(&self) -> Option<super::FlowchartEdgeLabelPadding> {
        match self {
            Self::Candidate { value, .. } => Some(*value),
            Self::Residual { .. } => None,
        }
    }
}

impl FlowchartScalarOutcome {
    const fn value(&self) -> Option<f32> {
        match self {
            Self::Candidate { value, .. } => Some(*value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug, PartialEq)]
enum FlowchartDasharrayOutcome {
    Candidate {
        rule_index: usize,
        value: String,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

#[derive(Debug)]
enum FlowchartTypographyOutcome {
    Candidate {
        rule_index: usize,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

impl FlowchartDasharrayOutcome {
    fn value(&self) -> Option<&str> {
        match self {
            Self::Candidate { value, .. } => Some(value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug)]
enum FlowchartEffectOutcome {
    Candidate {
        key: FamilyThemeMechanismKey,
        effect: SvgShadowEffect,
    },
    Cleared {
        rule_index: usize,
    },
    Residual {
        key: FamilyThemeMechanismKey,
    },
}

impl FlowchartEffectOutcome {
    fn effect(&self) -> Option<&SvgShadowEffect> {
        match self {
            Self::Candidate { effect, .. } => Some(effect),
            Self::Cleared { .. } | Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug)]
enum FlowchartOrdinalPaletteOutcome {
    Candidate { value: String },
    Residual { reason: FamilyThemeResidualReason },
}

impl FlowchartOrdinalPaletteOutcome {
    fn value(&self) -> Option<&str> {
        match self {
            Self::Candidate { value } => Some(value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartSourceFacetStatus {
    Absent,
    Admitted,
    Unverified,
}

impl FlowchartSourceFacetStatus {
    pub(crate) const fn from_parts(declared: bool, admitted: bool) -> Self {
        if admitted {
            Self::Admitted
        } else if declared {
            Self::Unverified
        } else {
            Self::Absent
        }
    }

    const fn overrides_theme(self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub(crate) const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }

    pub(crate) const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unverified, _) | (_, Self::Unverified) => Self::Unverified,
            (Self::Admitted, _) | (_, Self::Admitted) => Self::Admitted,
            (Self::Absent, Self::Absent) => Self::Absent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlowchartFacetPrecedence {
    source: FlowchartSourceFacetStatus,
    mermaid_config_overrides_typed: bool,
}

impl FlowchartFacetPrecedence {
    pub(crate) const fn new(
        source: FlowchartSourceFacetStatus,
        mermaid_config_overrides_typed: bool,
    ) -> Self {
        Self {
            source,
            mermaid_config_overrides_typed,
        }
    }

    const fn overrides_theme(self) -> bool {
        self.source.overrides_theme() || self.mermaid_config_overrides_typed
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartThemeFacetEmission {
    precedence: FlowchartFacetPrecedence,
    verified: bool,
}

impl FlowchartThemeFacetEmission {
    pub(crate) const fn new(precedence: FlowchartFacetPrecedence, verified: bool) -> Self {
        Self {
            precedence,
            verified,
        }
    }

    #[cfg(test)]
    const fn absent() -> Self {
        Self::new(
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, false),
            false,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlowchartShapeFacetEmissionReceipt {
    pub(crate) fill: bool,
    pub(crate) stroke: bool,
    pub(crate) stroke_width: bool,
    pub(crate) stroke_dasharray: bool,
    pub(crate) remaining_shape_style: bool,
}

impl FlowchartShapeFacetEmissionReceipt {
    pub(crate) const fn none() -> Self {
        Self {
            fill: false,
            stroke: false,
            stroke_width: false,
            stroke_dasharray: false,
            remaining_shape_style: false,
        }
    }

    pub(crate) const fn all() -> Self {
        Self {
            fill: true,
            stroke: true,
            stroke_width: true,
            stroke_dasharray: true,
            remaining_shape_style: true,
        }
    }

    pub(crate) const fn merge(self, other: Self) -> Self {
        Self {
            fill: self.fill || other.fill,
            stroke: self.stroke || other.stroke,
            stroke_width: self.stroke_width || other.stroke_width,
            stroke_dasharray: self.stroke_dasharray || other.stroke_dasharray,
            remaining_shape_style: self.remaining_shape_style || other.remaining_shape_style,
        }
    }

    pub(crate) fn verifies(self, property: &str) -> bool {
        match property {
            "fill" => self.fill,
            "stroke" | "stroke-linecap" | "stroke-linejoin" | "stroke-opacity" => self.stroke,
            "stroke-width" => self.stroke_width,
            "stroke-dasharray" => self.stroke_dasharray,
            "rx" | "ry" | "opacity" | "fill-opacity" => self.remaining_shape_style,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartNodeThemeEmission {
    pub(crate) fill: FlowchartThemeFacetEmission,
    pub(crate) stroke: FlowchartThemeFacetEmission,
    pub(crate) stroke_width: FlowchartThemeFacetEmission,
    pub(crate) stroke_dasharray: FlowchartThemeFacetEmission,
    pub(crate) radius: FlowchartThemeFacetEmission,
    pub(crate) effect: FlowchartThemeFacetEmission,
    pub(crate) label_fill: Option<FlowchartThemeFacetEmission>,
    pub(crate) font_stack: Option<FlowchartThemeFacetEmission>,
    pub(crate) font_size: Option<FlowchartThemeFacetEmission>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartEdgeThemeEmission {
    pub(crate) stroke: FlowchartThemeFacetEmission,
    pub(crate) stroke_width: FlowchartThemeFacetEmission,
    pub(crate) stroke_dasharray: FlowchartThemeFacetEmission,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartEdgeLabelThemeEmission {
    pub(crate) font_stack: Option<FlowchartThemeFacetEmission>,
    pub(crate) font_size: Option<FlowchartThemeFacetEmission>,
    pub(crate) padding: Option<FlowchartThemeFacetEmission>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartClusterThemeEmission {
    pub(crate) fill: FlowchartThemeFacetEmission,
    pub(crate) stroke: FlowchartThemeFacetEmission,
}

impl FlowchartNodeThemeEmission {
    #[cfg(test)]
    const fn none() -> Self {
        Self {
            fill: FlowchartThemeFacetEmission::absent(),
            stroke: FlowchartThemeFacetEmission::absent(),
            stroke_width: FlowchartThemeFacetEmission::absent(),
            stroke_dasharray: FlowchartThemeFacetEmission::absent(),
            radius: FlowchartThemeFacetEmission::absent(),
            effect: FlowchartThemeFacetEmission::absent(),
            label_fill: None,
            font_stack: None,
            font_size: None,
        }
    }
}

#[derive(Debug, Default)]
struct FlowchartLabelThemeStyle {
    fill: Option<FlowchartPaintOutcome>,
    font_stack: Option<FlowchartTypographyOutcome>,
    font_size: Option<FlowchartTypographyOutcome>,
    padding: Option<FlowchartPaddingOutcome>,
    matched_rules: MatchedThemeRules,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartEdgeThemeStyle {
    ordinal_theme: Option<ResolvedDiagramTheme>,
    stroke: Option<FlowchartPaintOutcome>,
    stroke_width: Option<FlowchartScalarOutcome>,
    stroke_dasharray: Option<FlowchartDasharrayOutcome>,
    matched_rules: MatchedThemeRules,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
    label: FlowchartLabelThemeStyle,
}

impl FlowchartEdgeThemeStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut resolved = Self::resolve_shape(theme, None, work_meter)?;
        work_meter.charge(theme.family_mechanism_routes().len())?;
        if theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Edge && rule.ordinal().is_some())
        {
            resolved.ordinal_theme = Some(theme.clone());
        }
        resolved.label =
            resolve_label_theme_style(theme, ThemeTarget::EdgeLabel, None, work_meter)?;
        Ok(resolved)
    }

    fn resolve_shape(
        theme: &ResolvedDiagramTheme,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            ordinal,
            work_meter,
        )?;
        let mut resolved = Self::default();
        resolved.matched_rules = style.take_matched_rules();

        // Fill is a semantic fallback for the stroke terminal, not a second SVG paint channel.
        // An explicit stroke owns that terminal even when its value is Clear or unsupported.
        let fill_is_stroke = matches!(
            style.stroke_resolution().specified(),
            Specified::Unspecified
        );
        for (property, origin) in style.winner_rule_properties() {
            let rule_index = origin.rule_index();
            match property {
                ResolvedStyleProperty::Stroke => {
                    resolved.stroke = resolve_paint(
                        theme,
                        rule_index,
                        style.stroke_resolution(),
                        FamilyThemeRuleFacet::stroke,
                    );
                }
                ResolvedStyleProperty::StrokeWidth => {
                    resolved.stroke_width =
                        resolve_stroke_width(theme, rule_index, style.stroke_width_resolution());
                }
                ResolvedStyleProperty::StrokeDasharray => {
                    resolved.stroke_dasharray = resolve_stroke_dasharray(
                        theme,
                        rule_index,
                        style.stroke_dasharray_resolution(),
                    );
                }
                ResolvedStyleProperty::Fill => {
                    if fill_is_stroke {
                        resolved.stroke = resolve_paint(
                            theme,
                            rule_index,
                            style.fill_resolution(),
                            FamilyThemeRuleFacet::fill,
                        );
                    }
                }
                ResolvedStyleProperty::Typography(property) => record_incomplete_edge_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Typography(property),
                ),
                ResolvedStyleProperty::Effect => record_incomplete_edge_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Effect,
                ),
                ResolvedStyleProperty::StrokeLinecap
                | ResolvedStyleProperty::StrokeLinejoin
                | ResolvedStyleProperty::Opacity
                | ResolvedStyleProperty::FillOpacity
                | ResolvedStyleProperty::StrokeOpacity
                | ResolvedStyleProperty::Radius
                | ResolvedStyleProperty::Padding => record_incomplete_edge_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

        Ok(resolved)
    }

    pub(crate) fn stroke_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        path_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && path_emitted)
            .then(|| self.stroke.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn stroke_dasharray_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        writer_supports_dasharray: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && writer_supports_dasharray)
            .then(|| {
                self.stroke_dasharray
                    .as_ref()
                    .and_then(FlowchartDasharrayOutcome::value)
            })
            .flatten()
    }

    pub(crate) fn stroke_width_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        writer_supports_stroke_width: bool,
    ) -> Option<f32> {
        (!precedence.overrides_theme() && writer_supports_stroke_width)
            .then(|| {
                self.stroke_width
                    .as_ref()
                    .and_then(FlowchartScalarOutcome::value)
            })
            .flatten()
    }

    pub(crate) fn font_stack_selected(&self, precedence: FlowchartFacetPrecedence) -> bool {
        !precedence.overrides_theme()
            && matches!(
                self.label.font_stack,
                Some(FlowchartTypographyOutcome::Candidate { .. })
            )
    }

    pub(crate) fn font_size_selected(&self, precedence: FlowchartFacetPrecedence) -> bool {
        !precedence.overrides_theme()
            && matches!(
                self.label.font_size,
                Some(FlowchartTypographyOutcome::Candidate { .. })
            )
    }

    pub(crate) fn edge_label_padding(&self) -> super::FlowchartEdgeLabelPadding {
        self.label
            .padding
            .as_ref()
            .and_then(FlowchartPaddingOutcome::value)
            .unwrap_or_default()
    }

    pub(crate) fn padding_selected(&self, precedence: FlowchartFacetPrecedence) -> bool {
        !precedence.overrides_theme()
            && matches!(
                self.label.padding,
                Some(FlowchartPaddingOutcome::Candidate { .. })
            )
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartClusterThemeStyle {
    fill: Option<FlowchartPaintOutcome>,
    stroke: Option<FlowchartPaintOutcome>,
    matched_rules: MatchedThemeRules,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
}

impl FlowchartClusterThemeStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            ordinal,
            work_meter,
        )?;
        let mut resolved = Self::default();
        resolved.matched_rules = style.take_matched_rules();

        for (property, origin) in style.winner_rule_properties() {
            let rule_index = origin.rule_index();
            match property {
                ResolvedStyleProperty::Fill => {
                    resolved.fill = resolve_paint(
                        theme,
                        rule_index,
                        style.fill_resolution(),
                        FamilyThemeRuleFacet::fill,
                    );
                }
                ResolvedStyleProperty::Stroke => {
                    resolved.stroke = resolve_paint(
                        theme,
                        rule_index,
                        style.stroke_resolution(),
                        FamilyThemeRuleFacet::stroke,
                    );
                }
                ResolvedStyleProperty::Typography(property) => record_incomplete_cluster_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Typography(property),
                ),
                ResolvedStyleProperty::Effect => record_incomplete_cluster_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Effect,
                ),
                ResolvedStyleProperty::StrokeWidth
                | ResolvedStyleProperty::StrokeDasharray
                | ResolvedStyleProperty::StrokeLinecap
                | ResolvedStyleProperty::StrokeLinejoin
                | ResolvedStyleProperty::Opacity
                | ResolvedStyleProperty::FillOpacity
                | ResolvedStyleProperty::StrokeOpacity
                | ResolvedStyleProperty::Radius
                | ResolvedStyleProperty::Padding => record_incomplete_cluster_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

        Ok(resolved)
    }

    pub(crate) fn fill_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| self.fill.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn stroke_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| self.stroke.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn append_inline_style(
        &self,
        out: &mut String,
        fill_precedence: FlowchartFacetPrecedence,
        stroke_precedence: FlowchartFacetPrecedence,
    ) {
        if let Some(fill) = self.fill_value(fill_precedence, true) {
            push_inline_declaration(out, "fill", fill);
        }
        if let Some(stroke) = self.stroke_value(stroke_precedence, true) {
            push_inline_declaration(out, "stroke", stroke);
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartClusterThemePlan<'a> {
    styles_by_id: FxHashMap<&'a str, FlowchartClusterThemeStyle>,
}

impl<'a> FlowchartClusterThemePlan<'a> {
    pub(crate) fn prepare(
        theme: Option<&ResolvedDiagramTheme>,
        semantic_ids: impl IntoIterator<Item = &'a str>,
        emitted_ids: impl IntoIterator<Item = &'a str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut emitted = FxHashSet::default();
        for id in emitted_ids {
            work_meter.charge(1)?;
            if !emitted.insert(id) {
                return Err(crate::Error::InvalidModel {
                    message: format!("Flowchart hierarchy emits cluster `{id}` more than once"),
                });
            }
        }

        let mut styles_by_id = FxHashMap::default();
        styles_by_id.reserve(emitted.len());
        let mut ordinal = 1usize;
        for id in semantic_ids {
            work_meter.charge(1)?;
            if !emitted.remove(id) {
                continue;
            }
            styles_by_id.insert(
                id,
                FlowchartClusterThemeStyle::resolve(theme, Some(ordinal), work_meter)?,
            );
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| work_meter.arithmetic_overflow())?;
        }

        if !emitted.is_empty() {
            let mut missing = emitted.into_iter().collect::<Vec<_>>();
            missing.sort_unstable();
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "Flowchart hierarchy emits clusters outside the semantic inventory: {}",
                    missing.join(", ")
                ),
            });
        }

        Ok(Self { styles_by_id })
    }

    pub(crate) fn style(&self, id: &str) -> crate::Result<&FlowchartClusterThemeStyle> {
        self.styles_by_id
            .get(id)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart Cluster theme style for `{id}`"),
            })
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartNodeThemeStyle {
    fill: Option<FlowchartPaintOutcome>,
    ordinal_palette_fill: Option<FlowchartOrdinalPaletteOutcome>,
    stroke: Option<FlowchartPaintOutcome>,
    stroke_width: Option<FlowchartScalarOutcome>,
    stroke_dasharray: Option<FlowchartDasharrayOutcome>,
    radius: Option<FlowchartScalarOutcome>,
    effect: Option<FlowchartEffectOutcome>,
    matched_rules: MatchedThemeRules,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
    label: FlowchartLabelThemeStyle,
}

impl FlowchartNodeThemeStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            ordinal,
            work_meter,
        )?;
        let mut resolved = Self::default();
        let mut fill_rule_present = false;
        resolved.matched_rules = style.take_matched_rules();

        for (property, origin) in style.winner_rule_properties() {
            let rule_index = origin.rule_index();
            match property {
                ResolvedStyleProperty::Fill => {
                    fill_rule_present = true;
                    resolved.fill = resolve_paint(
                        theme,
                        rule_index,
                        style.fill_resolution(),
                        FamilyThemeRuleFacet::fill,
                    );
                }
                ResolvedStyleProperty::Stroke => {
                    resolved.stroke = resolve_paint(
                        theme,
                        rule_index,
                        style.stroke_resolution(),
                        FamilyThemeRuleFacet::stroke,
                    );
                }
                ResolvedStyleProperty::StrokeWidth => {
                    resolved.stroke_width =
                        resolve_stroke_width(theme, rule_index, style.stroke_width_resolution());
                }
                ResolvedStyleProperty::StrokeDasharray => {
                    resolved.stroke_dasharray = resolve_stroke_dasharray(
                        theme,
                        rule_index,
                        style.stroke_dasharray_resolution(),
                    );
                }
                ResolvedStyleProperty::Radius => {
                    resolved.radius = resolve_radius(theme, rule_index, style.radius_resolution());
                }
                ResolvedStyleProperty::Typography(property) => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Typography(property),
                ),
                ResolvedStyleProperty::Effect => {}
                ResolvedStyleProperty::StrokeLinecap
                | ResolvedStyleProperty::StrokeLinejoin
                | ResolvedStyleProperty::Opacity
                | ResolvedStyleProperty::FillOpacity
                | ResolvedStyleProperty::StrokeOpacity
                | ResolvedStyleProperty::Padding => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

        resolved.effect = resolve_node_effect(theme, style.effect_resolution());
        if let Some(effect) = resolved.effect() {
            work_meter.charge(effect.stages().len())?;
        }
        resolved.label =
            resolve_label_theme_style(theme, ThemeTarget::NodeLabel, ordinal, work_meter)?;

        if !fill_rule_present
            && theme.ordinal_palette_disposition(ThemeTarget::Node)
                == Some(FamilyThemeDisposition::TypedAdapter)
        {
            resolved.ordinal_palette_fill = Some(match ordinal {
                Some(ordinal) => match theme.series_color(ThemeTarget::Node, ordinal) {
                    Some(color) => FlowchartOrdinalPaletteOutcome::Candidate {
                        value: color.as_css(),
                    },
                    None => FlowchartOrdinalPaletteOutcome::Residual {
                        reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                    },
                },
                None => FlowchartOrdinalPaletteOutcome::Residual {
                    reason: FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                },
            });
        }

        Ok(resolved)
    }

    pub(crate) fn effect(&self) -> Option<&SvgShadowEffect> {
        self.effect
            .as_ref()
            .and_then(FlowchartEffectOutcome::effect)
    }

    pub(crate) fn fill_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| {
                self.fill
                    .as_ref()
                    .and_then(FlowchartPaintOutcome::value)
                    .or_else(|| {
                        self.ordinal_palette_fill
                            .as_ref()
                            .and_then(FlowchartOrdinalPaletteOutcome::value)
                    })
            })
            .flatten()
    }

    pub(crate) fn stroke_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| self.stroke.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn stroke_width_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<f32> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| {
                self.stroke_width
                    .as_ref()
                    .and_then(FlowchartScalarOutcome::value)
            })
            .flatten()
    }

    pub(crate) fn radius_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<f32> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| self.radius.as_ref().and_then(FlowchartScalarOutcome::value))
            .flatten()
    }

    pub(crate) fn stroke_dasharray_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| {
                self.stroke_dasharray
                    .as_ref()
                    .and_then(FlowchartDasharrayOutcome::value)
            })
            .flatten()
    }

    pub(crate) fn font_stack_selected(&self, precedence: FlowchartFacetPrecedence) -> bool {
        !precedence.overrides_theme()
            && matches!(
                self.label.font_stack,
                Some(FlowchartTypographyOutcome::Candidate { .. })
            )
    }

    pub(crate) fn font_size_selected(&self, precedence: FlowchartFacetPrecedence) -> bool {
        !precedence.overrides_theme()
            && matches!(
                self.label.font_size,
                Some(FlowchartTypographyOutcome::Candidate { .. })
            )
    }

    pub(crate) fn label_fill_value(
        &self,
        precedence: FlowchartFacetPrecedence,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!precedence.overrides_theme() && channel_emitted)
            .then(|| {
                self.label
                    .fill
                    .as_ref()
                    .and_then(FlowchartPaintOutcome::value)
            })
            .flatten()
    }

    pub(crate) fn has_label_fill_route(&self) -> bool {
        self.label.fill.is_some()
    }

    pub(crate) fn append_inline_style(
        &self,
        out: &mut String,
        fill_precedence: FlowchartFacetPrecedence,
        stroke_precedence: FlowchartFacetPrecedence,
        stroke_width_precedence: FlowchartFacetPrecedence,
        stroke_dasharray_precedence: FlowchartFacetPrecedence,
        fill_emitted: bool,
        stroke_emitted: bool,
        stroke_width_emitted: bool,
        stroke_dasharray_emitted: bool,
    ) {
        if let Some(fill) = self.fill_value(fill_precedence, fill_emitted) {
            push_inline_declaration(out, "fill", fill);
        }
        if let Some(stroke) = self.stroke_value(stroke_precedence, stroke_emitted) {
            push_inline_declaration(out, "stroke", stroke);
        }
        if let Some(stroke_width) =
            self.stroke_width_value(stroke_width_precedence, stroke_width_emitted)
        {
            push_inline_declaration(out, "stroke-width", &format!("{stroke_width}px"));
        }
        if let Some(stroke_dasharray) =
            self.stroke_dasharray_value(stroke_dasharray_precedence, stroke_dasharray_emitted)
        {
            push_inline_declaration(out, "stroke-dasharray", stroke_dasharray);
        }
    }
}

fn resolve_label_theme_style(
    theme: &ResolvedDiagramTheme,
    target: ThemeTarget,
    ordinal: Option<usize>,
    work_meter: &OperationWorkMeter,
) -> Result<FlowchartLabelThemeStyle, OperationWorkError> {
    let mut style =
        theme.style_with_work_meter(target, ThemeVariant::Default, ordinal, work_meter)?;
    let mut resolved = FlowchartLabelThemeStyle::default();
    resolved.matched_rules = style.take_matched_rules();
    for (property, origin) in style.winner_rule_properties() {
        let rule_index = origin.rule_index();
        match property {
            ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontStack) => {
                resolved.font_stack = resolve_typography(
                    theme,
                    rule_index,
                    &style.typography_resolution().patch().font_stack,
                    ThemeTypographyProperty::FontStack,
                );
            }
            ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize) => {
                resolved.font_size = resolve_typography(
                    theme,
                    rule_index,
                    &style.typography_resolution().patch().font_size_px,
                    ThemeTypographyProperty::FontSize,
                );
            }
            ResolvedStyleProperty::Padding => {
                resolved.padding = resolve_padding(theme, rule_index, style.padding_resolution());
            }
            ResolvedStyleProperty::Typography(property) => record_incomplete_label_facet(
                theme,
                &mut resolved,
                rule_index,
                FamilyThemeRuleFacet::Typography(property),
            ),
            ResolvedStyleProperty::Fill => {
                if let Some(facet) = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())
                {
                    resolved.fill = resolve_paint(
                        theme,
                        rule_index,
                        style.fill_resolution(),
                        FamilyThemeRuleFacet::fill,
                    );
                    if resolved.fill.is_none()
                        && theme.rule_facet_disposition(rule_index, facet)
                            == Some(FamilyThemeDisposition::TypedAdapter)
                    {
                        resolved.incomplete_rules.insert(rule_index);
                    }
                } else {
                    resolved.incomplete_rules.insert(rule_index);
                }
            }
            ResolvedStyleProperty::Stroke => record_incomplete_label_facet(
                theme,
                &mut resolved,
                rule_index,
                FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())
                    .unwrap_or_else(|| panic!("winning {target:?} stroke has a concrete facet")),
            ),
            ResolvedStyleProperty::Effect => record_incomplete_label_facet(
                theme,
                &mut resolved,
                rule_index,
                FamilyThemeRuleFacet::Effect,
            ),
            ResolvedStyleProperty::StrokeWidth
            | ResolvedStyleProperty::StrokeDasharray
            | ResolvedStyleProperty::StrokeLinecap
            | ResolvedStyleProperty::StrokeLinejoin
            | ResolvedStyleProperty::Opacity
            | ResolvedStyleProperty::FillOpacity
            | ResolvedStyleProperty::StrokeOpacity
            | ResolvedStyleProperty::Radius => record_incomplete_label_facet(
                theme,
                &mut resolved,
                rule_index,
                non_paint_facet(property),
            ),
        }
    }
    Ok(resolved)
}

fn resolve_padding(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<crate::diagram_theme::InsetsPx>,
) -> Option<FlowchartPaddingOutcome> {
    let facet = match property.specified() {
        Specified::Unspecified => return None,
        Specified::Clear | Specified::Value(_) => FamilyThemeRuleFacet::Padding,
    };
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartPaddingOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedGeometry,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(value) => Some(FlowchartPaddingOutcome::Candidate {
                rule_index,
                value: super::FlowchartEdgeLabelPadding::from_insets(*value),
            }),
            Specified::Clear | Specified::Unspecified => Some(FlowchartPaddingOutcome::Residual {
                rule_index,
                reason: FamilyThemeResidualReason::UnsupportedGeometry,
            }),
        },
    }
}

fn resolve_paint(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<CanvasPaint>,
    facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
) -> Option<FlowchartPaintOutcome> {
    let facet = facet(property.specified())?;
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartPaintOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(CanvasPaint::Transparent) => Some(FlowchartPaintOutcome::Candidate {
                rule_index,
                value: "none".to_string(),
            }),
            Specified::Value(CanvasPaint::Solid(color)) => Some(FlowchartPaintOutcome::Candidate {
                rule_index,
                value: color.as_css(),
            }),
            Specified::Unspecified
            | Specified::Clear
            | Specified::Value(
                CanvasPaint::LinearGradient(_)
                | CanvasPaint::RadialGradient(_)
                | CanvasPaint::Pattern(_),
            ) => Some(FlowchartPaintOutcome::Residual {
                rule_index,
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }),
        },
    }
}

fn resolve_stroke_width(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<f32>,
) -> Option<FlowchartScalarOutcome> {
    let facet = match property.specified() {
        Specified::Unspecified => return None,
        Specified::Clear | Specified::Value(_) => FamilyThemeRuleFacet::StrokeWidth,
    };
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartScalarOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedGeometry,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(value) => Some(FlowchartScalarOutcome::Candidate {
                rule_index,
                value: *value,
            }),
            Specified::Clear | Specified::Unspecified => Some(FlowchartScalarOutcome::Residual {
                rule_index,
                reason: FamilyThemeResidualReason::UnsupportedGeometry,
            }),
        },
    }
}

fn resolve_stroke_dasharray(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<Vec<f32>>,
) -> Option<FlowchartDasharrayOutcome> {
    let facet = match property.specified() {
        Specified::Unspecified => return None,
        Specified::Clear | Specified::Value(_) => FamilyThemeRuleFacet::StrokeDasharray,
    };
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartDasharrayOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedGeometry,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(values) => Some(FlowchartDasharrayOutcome::Candidate {
                rule_index,
                value: values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" "),
            }),
            Specified::Clear | Specified::Unspecified => {
                Some(FlowchartDasharrayOutcome::Residual {
                    rule_index,
                    reason: FamilyThemeResidualReason::UnsupportedGeometry,
                })
            }
        },
    }
}

fn resolve_radius(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<f32>,
) -> Option<FlowchartScalarOutcome> {
    let facet = match property.specified() {
        Specified::Unspecified => return None,
        Specified::Clear | Specified::Value(_) => FamilyThemeRuleFacet::Radius,
    };
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartScalarOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedGeometry,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(value) => Some(FlowchartScalarOutcome::Candidate {
                rule_index,
                value: *value,
            }),
            Specified::Clear | Specified::Unspecified => Some(FlowchartScalarOutcome::Residual {
                rule_index,
                reason: FamilyThemeResidualReason::UnsupportedGeometry,
            }),
        },
    }
}

fn resolve_typography<T>(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &Specified<T>,
    typography_property: ThemeTypographyProperty,
) -> Option<FlowchartTypographyOutcome> {
    let facet = match property {
        Specified::Unspecified => return None,
        Specified::Clear | Specified::Value(_) => {
            FamilyThemeRuleFacet::Typography(typography_property)
        }
    };
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartTypographyOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedTypography,
        }),
        FamilyThemeDisposition::TypedAdapter => match property {
            Specified::Value(_) => Some(FlowchartTypographyOutcome::Candidate { rule_index }),
            Specified::Clear | Specified::Unspecified => {
                Some(FlowchartTypographyOutcome::Residual {
                    rule_index,
                    reason: FamilyThemeResidualReason::UnsupportedTypography,
                })
            }
        },
    }
}

fn resolve_node_effect(
    theme: &ResolvedDiagramTheme,
    property: &ResolvedProperty<String>,
) -> Option<FlowchartEffectOutcome> {
    let (key, graph, disposition) = match theme.resolve_effect(ThemeTarget::Node, property)? {
        ResolvedThemeEffect::ClearedByRule => {
            let rule_index = property.winner()?.rule_index();
            return match theme.rule_facet_disposition(rule_index, FamilyThemeRuleFacet::Effect)? {
                FamilyThemeDisposition::TypedAdapter => {
                    Some(FlowchartEffectOutcome::Cleared { rule_index })
                }
                FamilyThemeDisposition::Unsupported => Some(FlowchartEffectOutcome::Residual {
                    key: FamilyThemeMechanismKey::Rule {
                        index: rule_index,
                        target: ThemeTarget::Node,
                    },
                }),
                FamilyThemeDisposition::LegacyCompatibility => None,
            };
        }
        ResolvedThemeEffect::Rule { graph } => {
            let rule_index = property.winner()?.rule_index();
            (
                FamilyThemeMechanismKey::Rule {
                    index: rule_index,
                    target: ThemeTarget::Node,
                },
                graph,
                theme.rule_facet_disposition(rule_index, FamilyThemeRuleFacet::Effect)?,
            )
        }
        ResolvedThemeEffect::Binding { graph, .. } => {
            let route = theme.family_mechanism_routes().iter().find(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Node,
                        ..
                    }
                )
            })?;
            (
                theme.family_mechanism_key(*route),
                graph,
                route.disposition(),
            )
        }
    };
    match disposition {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::TypedAdapter => {
            Some(match graph.and_then(SvgShadowEffect::from_graph) {
                Some(effect) => FlowchartEffectOutcome::Candidate { key, effect },
                None => FlowchartEffectOutcome::Residual { key },
            })
        }
        FamilyThemeDisposition::Unsupported => Some(FlowchartEffectOutcome::Residual { key }),
    }
}

fn non_paint_facet(property: ResolvedStyleProperty) -> FamilyThemeRuleFacet {
    match property {
        ResolvedStyleProperty::StrokeWidth => FamilyThemeRuleFacet::StrokeWidth,
        ResolvedStyleProperty::StrokeDasharray => FamilyThemeRuleFacet::StrokeDasharray,
        ResolvedStyleProperty::StrokeLinecap => FamilyThemeRuleFacet::StrokeLinecap,
        ResolvedStyleProperty::StrokeLinejoin => FamilyThemeRuleFacet::StrokeLinejoin,
        ResolvedStyleProperty::Opacity => FamilyThemeRuleFacet::Opacity,
        ResolvedStyleProperty::FillOpacity => FamilyThemeRuleFacet::FillOpacity,
        ResolvedStyleProperty::StrokeOpacity => FamilyThemeRuleFacet::StrokeOpacity,
        ResolvedStyleProperty::Radius => FamilyThemeRuleFacet::Radius,
        ResolvedStyleProperty::Padding => FamilyThemeRuleFacet::Padding,
        _ => unreachable!("non-paint facet mapping received a paint property"),
    }
}

fn record_incomplete_facet(
    theme: &ResolvedDiagramTheme,
    resolved: &mut FlowchartNodeThemeStyle,
    rule_index: usize,
    facet: FamilyThemeRuleFacet,
) {
    match theme.rule_facet_disposition(rule_index, facet) {
        None | Some(FamilyThemeDisposition::LegacyCompatibility) => {}
        Some(FamilyThemeDisposition::TypedAdapter) => {
            resolved.incomplete_rules.insert(rule_index);
        }
        Some(FamilyThemeDisposition::Unsupported) => {
            resolved
                .residual_rules
                .entry(rule_index)
                .or_insert(unsupported_reason_for_facet(facet));
        }
    }
}

fn record_incomplete_label_facet(
    theme: &ResolvedDiagramTheme,
    resolved: &mut FlowchartLabelThemeStyle,
    rule_index: usize,
    facet: FamilyThemeRuleFacet,
) {
    match theme.rule_facet_disposition(rule_index, facet) {
        None | Some(FamilyThemeDisposition::LegacyCompatibility) => {}
        Some(FamilyThemeDisposition::TypedAdapter) => {
            resolved.incomplete_rules.insert(rule_index);
        }
        Some(FamilyThemeDisposition::Unsupported) => {
            resolved
                .residual_rules
                .entry(rule_index)
                .or_insert(unsupported_reason_for_facet(facet));
        }
    }
}

fn record_incomplete_edge_facet(
    theme: &ResolvedDiagramTheme,
    resolved: &mut FlowchartEdgeThemeStyle,
    rule_index: usize,
    facet: FamilyThemeRuleFacet,
) {
    match theme.rule_facet_disposition(rule_index, facet) {
        None | Some(FamilyThemeDisposition::LegacyCompatibility) => {}
        Some(FamilyThemeDisposition::TypedAdapter) => {
            resolved.incomplete_rules.insert(rule_index);
        }
        Some(FamilyThemeDisposition::Unsupported) => {
            resolved
                .residual_rules
                .entry(rule_index)
                .or_insert(unsupported_reason_for_facet(facet));
        }
    }
}

fn record_incomplete_cluster_facet(
    theme: &ResolvedDiagramTheme,
    resolved: &mut FlowchartClusterThemeStyle,
    rule_index: usize,
    facet: FamilyThemeRuleFacet,
) {
    match theme.rule_facet_disposition(rule_index, facet) {
        None | Some(FamilyThemeDisposition::LegacyCompatibility) => {}
        Some(FamilyThemeDisposition::TypedAdapter) => {
            resolved.incomplete_rules.insert(rule_index);
        }
        Some(FamilyThemeDisposition::Unsupported) => {
            resolved
                .residual_rules
                .entry(rule_index)
                .or_insert(unsupported_reason_for_facet(facet));
        }
    }
}

fn push_inline_declaration(out: &mut String, property: &str, value: &str) {
    if !out.is_empty() {
        out.push(';');
    }
    let _ = write!(out, "{property}:{value} !important");
}

#[derive(Debug, Default)]
struct FlowchartRuleEvidenceState {
    emitted: bool,
    matched_rules: BTreeSet<usize>,
    seen_static_blocks: Vec<Arc<[usize]>>,
    applied_rules: BTreeSet<usize>,
    applied_rule_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    unverified_rules: BTreeSet<usize>,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
}

impl FlowchartRuleEvidenceState {
    fn unseen_static_blocks<'a>(
        &'a self,
        rules: &'a MatchedThemeRules,
    ) -> impl Iterator<Item = &'a Arc<[usize]>> {
        rules.static_blocks().iter().filter(|block| {
            !self
                .seen_static_blocks
                .iter()
                .any(|seen| Arc::ptr_eq(seen, block))
        })
    }

    fn provenance_work(&self, rules: &MatchedThemeRules) -> usize {
        rules.dynamic_indices().len()
            + self
                .unseen_static_blocks(rules)
                .map(|block| block.len())
                .sum::<usize>()
    }

    /// Called only after the complete emission's provenance charge succeeds.
    fn record_matches(&mut self, rules: &MatchedThemeRules) {
        for block in rules.static_blocks() {
            if !self
                .seen_static_blocks
                .iter()
                .any(|seen| Arc::ptr_eq(seen, block))
            {
                self.matched_rules.extend(block.iter().copied());
                self.seen_static_blocks.push(Arc::clone(block));
            }
        }
        self.matched_rules
            .extend(rules.dynamic_indices().iter().copied());
    }
}

#[derive(Debug, Default)]
struct FlowchartThemeEvidenceState {
    node: FlowchartRuleEvidenceState,
    node_label: FlowchartRuleEvidenceState,
    edge: FlowchartRuleEvidenceState,
    edge_ordinal: usize,
    edge_label: FlowchartRuleEvidenceState,
    cluster: FlowchartRuleEvidenceState,
    title: Option<FamilyThemeEvidence>,
    marker: Option<FamilyThemeEvidence>,
    node_ordinal_palette: FlowchartMechanismObservation,
    node_effect_bindings: BTreeMap<FamilyThemeMechanismKey, FlowchartMechanismObservation>,
    source_residuals: BTreeMap<FlowchartSourceResidualKey, SourceStyleResidual>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum FlowchartMechanismObservation {
    #[default]
    NotObserved,
    Applied,
    Residual(FamilyThemeResidualReason),
}

impl FlowchartMechanismObservation {
    fn observe_applied(&mut self) {
        if matches!(self, Self::NotObserved) {
            *self = Self::Applied;
        }
    }

    fn observe_residual(&mut self, reason: FamilyThemeResidualReason) {
        if !matches!(self, Self::Residual(_)) {
            *self = Self::Residual(reason);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FlowchartSourceResidualKey {
    raw: Arc<str>,
    property: Option<Arc<str>>,
    owner_id: Arc<str>,
    class_id: Option<Arc<str>>,
    origin: &'static str,
    channel: &'static str,
    assignment_ordinal: Option<usize>,
    declaration_ordinal: usize,
    reason: &'static str,
}

impl From<&SourceStyleResidual> for FlowchartSourceResidualKey {
    fn from(residual: &SourceStyleResidual) -> Self {
        let provenance = residual.provenance();
        Self {
            raw: residual.raw_arc(),
            property: residual.property_arc(),
            owner_id: provenance.owner_id_arc(),
            class_id: provenance.class_id_arc(),
            origin: provenance.origin().id(),
            channel: provenance.channel().id(),
            assignment_ordinal: provenance.assignment_ordinal(),
            declaration_ordinal: provenance.declaration_ordinal(),
            reason: residual.reason().id(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartThemeEvidenceRecorder {
    state: Mutex<FlowchartThemeEvidenceState>,
}

impl FlowchartThemeEvidenceRecorder {
    pub(crate) fn record_cluster_emission(
        &self,
        style: &FlowchartClusterThemeStyle,
        emission: FlowchartClusterThemeEmission,
        source_residuals: &[SourceStyleResidual],
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        work_meter.charge_emit_work(state.cluster.provenance_work(&style.matched_rules))?;
        state.cluster.emitted = true;
        state.cluster.record_matches(&style.matched_rules);
        for (rule_index, reason) in &style.residual_rules {
            state
                .cluster
                .residual_rules
                .entry(*rule_index)
                .or_insert(*reason);
        }
        record_paint_outcome(
            &mut state.cluster,
            style.fill.as_ref(),
            emission.fill.precedence,
            emission.fill.verified,
        );
        record_paint_outcome(
            &mut state.cluster,
            style.stroke.as_ref(),
            emission.stroke.precedence,
            emission.stroke.verified,
        );
        state
            .cluster
            .incomplete_rules
            .extend(style.incomplete_rules.iter().copied());
        insert_source_residuals(&mut state, source_residuals);
        Ok(())
    }

    pub(crate) fn record_node_emission(
        &self,
        style: &FlowchartNodeThemeStyle,
        emission: FlowchartNodeThemeEmission,
        source_residuals: &[SourceStyleResidual],
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let label_emitted = emission.label_fill.is_some()
            || emission.font_stack.is_some()
            || emission.font_size.is_some();
        let work = state.node.provenance_work(&style.matched_rules)
            + if label_emitted {
                state.node_label.provenance_work(&style.label.matched_rules)
            } else {
                0
            };
        work_meter.charge_emit_work(work)?;
        state.node.emitted = true;
        state.node.record_matches(&style.matched_rules);
        for (rule_index, reason) in &style.residual_rules {
            state
                .node
                .residual_rules
                .entry(*rule_index)
                .or_insert(*reason);
        }
        record_paint_outcome(
            &mut state.node,
            style.fill.as_ref(),
            emission.fill.precedence,
            emission.fill.verified,
        );
        record_ordinal_palette_outcome(
            &mut state.node_ordinal_palette,
            style.ordinal_palette_fill.as_ref(),
            emission.fill.precedence,
            emission.fill.verified,
        );
        record_paint_outcome(
            &mut state.node,
            style.stroke.as_ref(),
            emission.stroke.precedence,
            emission.stroke.verified,
        );
        record_scalar_outcome(
            &mut state.node,
            style.stroke_width.as_ref(),
            emission.stroke_width.precedence,
            emission.stroke_width.verified,
            ThemeCapability::BorderStyling,
        );
        record_dasharray_outcome(
            &mut state.node,
            style.stroke_dasharray.as_ref(),
            emission.stroke_dasharray.precedence,
            emission.stroke_dasharray.verified,
        );
        record_scalar_outcome(
            &mut state.node,
            style.radius.as_ref(),
            emission.radius.precedence,
            emission.radius.verified,
            ThemeCapability::RoundedGeometry,
        );
        record_node_effect_outcome(&mut state, style.effect.as_ref(), emission.effect);
        state
            .node
            .incomplete_rules
            .extend(style.incomplete_rules.iter().copied());
        if label_emitted {
            state.node_label.emitted = true;
            state.node_label.record_matches(&style.label.matched_rules);
            for (rule_index, reason) in &style.label.residual_rules {
                state
                    .node_label
                    .residual_rules
                    .entry(*rule_index)
                    .or_insert(*reason);
            }
            if let Some(label_fill_emission) = emission.label_fill {
                record_paint_outcome(
                    &mut state.node_label,
                    style.label.fill.as_ref(),
                    label_fill_emission.precedence,
                    label_fill_emission.verified,
                );
            }
            if let Some(font_stack_emission) = emission.font_stack {
                record_typography_outcome(
                    &mut state.node_label,
                    style.label.font_stack.as_ref(),
                    font_stack_emission.precedence,
                    font_stack_emission.verified,
                );
            }
            if let Some(font_size_emission) = emission.font_size {
                record_typography_outcome(
                    &mut state.node_label,
                    style.label.font_size.as_ref(),
                    font_size_emission.precedence,
                    font_size_emission.verified,
                );
            }
            state
                .node_label
                .incomplete_rules
                .extend(style.label.incomplete_rules.iter().copied());
        }
        insert_source_residuals(&mut state, source_residuals);
        Ok(())
    }

    pub(crate) fn record_source_residuals(&self, source_residuals: &[SourceStyleResidual]) {
        if source_residuals.is_empty() {
            return;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        insert_source_residuals(&mut state, source_residuals);
    }

    pub(crate) fn record_edge_emission(
        &self,
        style: &FlowchartEdgeThemeStyle,
        emission: FlowchartEdgeThemeEmission,
        source_residuals: &[SourceStyleResidual],
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ordinal = state.edge_ordinal + 1;
        let emitted_style = style;
        let ordinal_style;
        let style = if let Some(theme) = &style.ordinal_theme {
            ordinal_style =
                FlowchartEdgeThemeStyle::resolve_shape(theme, Some(ordinal), work_meter)?;
            &ordinal_style
        } else {
            style
        };
        work_meter.charge_emit_work(state.edge.provenance_work(&style.matched_rules))?;
        state.edge_ordinal = ordinal;
        state.edge.emitted = true;
        state.edge.record_matches(&style.matched_rules);
        for (rule_index, reason) in &style.residual_rules {
            state
                .edge
                .residual_rules
                .entry(*rule_index)
                .or_insert(*reason);
        }
        // The writer receipt belongs to the static style it actually emitted. Ordinal
        // reconciliation must not reuse that receipt for a different winning rule or value.
        let static_only = emitted_style.ordinal_theme.is_none();
        record_edge_paint_outcome(
            &mut state.edge,
            style.stroke.as_ref(),
            emission.stroke.precedence,
            emission.stroke.verified && (static_only || style.stroke == emitted_style.stroke),
        );
        record_scalar_outcome(
            &mut state.edge,
            style.stroke_width.as_ref(),
            emission.stroke_width.precedence,
            emission.stroke_width.verified
                && (static_only || style.stroke_width == emitted_style.stroke_width),
            ThemeCapability::BorderStyling,
        );
        record_dasharray_outcome(
            &mut state.edge,
            style.stroke_dasharray.as_ref(),
            emission.stroke_dasharray.precedence,
            emission.stroke_dasharray.verified
                && (static_only || style.stroke_dasharray == emitted_style.stroke_dasharray),
        );
        state
            .edge
            .incomplete_rules
            .extend(style.incomplete_rules.iter().copied());
        insert_source_residuals(&mut state, source_residuals);
        Ok(())
    }

    pub(crate) fn record_edge_label_emission(
        &self,
        style: &FlowchartEdgeThemeStyle,
        emission: FlowchartEdgeLabelThemeEmission,
        source_residuals: &[SourceStyleResidual],
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let label_emitted = emission.font_stack.is_some()
            || emission.font_size.is_some()
            || emission.padding.is_some();
        work_meter.charge_emit_work(if label_emitted {
            state.edge_label.provenance_work(&style.label.matched_rules)
        } else {
            0
        })?;
        if label_emitted {
            state.edge_label.emitted = true;
            state.edge_label.record_matches(&style.label.matched_rules);
            for (rule_index, reason) in &style.label.residual_rules {
                state
                    .edge_label
                    .residual_rules
                    .entry(*rule_index)
                    .or_insert(*reason);
            }
            if let Some(font_stack_emission) = emission.font_stack {
                record_typography_outcome(
                    &mut state.edge_label,
                    style.label.font_stack.as_ref(),
                    font_stack_emission.precedence,
                    font_stack_emission.verified,
                );
            }
            if let Some(font_size_emission) = emission.font_size {
                record_typography_outcome(
                    &mut state.edge_label,
                    style.label.font_size.as_ref(),
                    font_size_emission.precedence,
                    font_size_emission.verified,
                );
            }
            if let Some(padding_emission) = emission.padding {
                record_padding_outcome(
                    &mut state.edge_label,
                    style.label.padding.as_ref(),
                    padding_emission.precedence,
                    padding_emission.verified,
                );
            }
            state
                .edge_label
                .incomplete_rules
                .extend(style.label.incomplete_rules.iter().copied());
        }
        insert_source_residuals(&mut state, source_residuals);
        Ok(())
    }

    pub(crate) fn record_marker_evidence(&self, evidence: FamilyThemeEvidence) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .marker = Some(evidence);
    }

    pub(crate) fn record_title_evidence(&self, evidence: FamilyThemeEvidence) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .title = Some(evidence);
    }

    pub(crate) fn finish(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> (FamilyThemeEvidence, Vec<SourceStyleResidual>) {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_residuals = state.source_residuals.values().cloned().collect();
        if let Some(title) = &state.title {
            evidence.merge_accounted_from(title.clone());
        }
        if let Some(marker) = &state.marker {
            evidence.merge_accounted_from(marker.clone());
        }
        let Some(theme) = theme else {
            return (evidence, source_residuals);
        };

        let mut rule_routes = BTreeSet::<(usize, ThemeTarget)>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index, target, ..
                } => {
                    rule_routes.insert((rule_index, target));
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Node,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !state.node.emitted {
                        evidence.mark_not_applicable(key);
                    } else {
                        match route.disposition() {
                            FamilyThemeDisposition::LegacyCompatibility => {
                                // The compatibility lane owns this route; its parse evidence is
                                // checked separately from typed family evidence.
                            }
                            FamilyThemeDisposition::TypedAdapter => {
                                match state.node_ordinal_palette {
                                    FlowchartMechanismObservation::NotObserved => {
                                        evidence.mark_not_applicable(key)
                                    }
                                    FlowchartMechanismObservation::Applied => evidence
                                        .mark_applied_with_capabilities(
                                            key,
                                            [ThemeCapability::SolidPaint],
                                        ),
                                    FlowchartMechanismObservation::Residual(reason) => {
                                        evidence.mark_residual(key, reason)
                                    }
                                }
                            }
                            FamilyThemeDisposition::Unsupported => evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            ),
                        }
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    match state
                        .node_effect_bindings
                        .get(&key)
                        .copied()
                        .unwrap_or_default()
                    {
                        FlowchartMechanismObservation::NotObserved => {
                            evidence.mark_not_applicable(key);
                        }
                        FlowchartMechanismObservation::Applied => evidence
                            .mark_applied_with_capabilities(
                                key,
                                [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                            ),
                        FlowchartMechanismObservation::Residual(reason) => {
                            evidence.mark_residual(key, reason);
                        }
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        for (rule_index, target) in rule_routes {
            let target_state = match target {
                ThemeTarget::Node => &state.node,
                ThemeTarget::NodeLabel => &state.node_label,
                ThemeTarget::Edge => &state.edge,
                ThemeTarget::EdgeLabel => &state.edge_label,
                ThemeTarget::Cluster => &state.cluster,
                _ => continue,
            };
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !target_state.emitted || !target_state.matched_rules.contains(&rule_index) {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = target_state.residual_rules.get(&rule_index).copied() {
                evidence.mark_residual(key, reason);
            } else if target_state.incomplete_rules.contains(&rule_index) {
                // A matching winner reached the family consumer, but this slice does not yet share
                // enough source-shadow information to classify the facet without guessing.
            } else if target_state.unverified_rules.contains(&rule_index) {
                // Every applicable target must reach a terminal writer. An applied path cannot
                // mask a second path whose consumption was not observed.
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
            } else if target_state.applied_rules.contains(&rule_index) {
                evidence.mark_applied_with_capabilities(
                    key,
                    target_state
                        .applied_rule_capabilities
                        .get(&rule_index)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        (evidence, source_residuals)
    }
}

fn insert_source_residuals(
    state: &mut FlowchartThemeEvidenceState,
    source_residuals: &[SourceStyleResidual],
) {
    for residual in source_residuals {
        state
            .source_residuals
            .entry(FlowchartSourceResidualKey::from(residual))
            .or_insert_with(|| residual.clone());
    }
}

fn record_paint_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartPaintOutcome>,
    precedence: FlowchartFacetPrecedence,
    channel_emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    if !channel_emitted {
        match outcome {
            Some(FlowchartPaintOutcome::Candidate { rule_index, .. }) => {
                state
                    .residual_rules
                    .entry(*rule_index)
                    .or_insert(FamilyThemeResidualReason::UnsupportedPaint);
            }
            Some(FlowchartPaintOutcome::Residual { rule_index, reason }) => {
                state.residual_rules.entry(*rule_index).or_insert(*reason);
            }
            None => {}
        }
        return;
    }
    match outcome {
        Some(FlowchartPaintOutcome::Candidate { rule_index, value }) => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(if value == "none" {
                    ThemeCapability::TransparentPaint
                } else {
                    ThemeCapability::SolidPaint
                });
        }
        Some(FlowchartPaintOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_edge_paint_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartPaintOutcome>,
    precedence: FlowchartFacetPrecedence,
    channel_emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartPaintOutcome::Candidate { rule_index, value }) if channel_emitted => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(if value == "none" {
                    ThemeCapability::TransparentPaint
                } else {
                    ThemeCapability::SolidPaint
                });
        }
        Some(FlowchartPaintOutcome::Candidate { rule_index, .. }) => {
            state.unverified_rules.insert(*rule_index);
        }
        Some(FlowchartPaintOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_scalar_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartScalarOutcome>,
    precedence: FlowchartFacetPrecedence,
    channel_emitted: bool,
    capability: ThemeCapability,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartScalarOutcome::Candidate { rule_index, .. }) if channel_emitted => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(capability);
        }
        Some(FlowchartScalarOutcome::Candidate { rule_index, .. }) => {
            state
                .residual_rules
                .entry(*rule_index)
                .or_insert(FamilyThemeResidualReason::UnsupportedGeometry);
        }
        Some(FlowchartScalarOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_padding_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartPaddingOutcome>,
    precedence: FlowchartFacetPrecedence,
    emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartPaddingOutcome::Candidate { rule_index, .. }) if emitted => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(ThemeCapability::ContentPadding);
        }
        Some(FlowchartPaddingOutcome::Candidate { rule_index, .. }) => {
            state
                .residual_rules
                .entry(*rule_index)
                .or_insert(FamilyThemeResidualReason::UnsupportedGeometry);
        }
        Some(FlowchartPaddingOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_dasharray_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartDasharrayOutcome>,
    precedence: FlowchartFacetPrecedence,
    channel_emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartDasharrayOutcome::Candidate { rule_index, .. }) if channel_emitted => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(ThemeCapability::DashStyling);
        }
        Some(FlowchartDasharrayOutcome::Candidate { rule_index, .. }) => {
            state
                .residual_rules
                .entry(*rule_index)
                .or_insert(FamilyThemeResidualReason::UnsupportedGeometry);
        }
        Some(FlowchartDasharrayOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_typography_outcome(
    state: &mut FlowchartRuleEvidenceState,
    outcome: Option<&FlowchartTypographyOutcome>,
    precedence: FlowchartFacetPrecedence,
    emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartTypographyOutcome::Candidate { rule_index }) if emitted => {
            state.applied_rules.insert(*rule_index);
            state
                .applied_rule_capabilities
                .entry(*rule_index)
                .or_default()
                .insert(ThemeCapability::Typography);
        }
        Some(FlowchartTypographyOutcome::Candidate { rule_index }) => {
            state
                .residual_rules
                .entry(*rule_index)
                .or_insert(FamilyThemeResidualReason::UnsupportedTypography);
        }
        Some(FlowchartTypographyOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

fn record_node_effect_outcome(
    state: &mut FlowchartThemeEvidenceState,
    outcome: Option<&FlowchartEffectOutcome>,
    emission: FlowchartThemeFacetEmission,
) {
    if emission.precedence.overrides_theme() {
        return;
    }
    let Some(outcome) = outcome else {
        return;
    };
    if let FlowchartEffectOutcome::Cleared { rule_index } = outcome {
        if emission.verified {
            state.node.applied_rules.insert(*rule_index);
        } else {
            state
                .node
                .residual_rules
                .entry(*rule_index)
                .or_insert(FamilyThemeResidualReason::UnsupportedEffect);
        }
        return;
    }
    let (key, applied) = match outcome {
        FlowchartEffectOutcome::Candidate { key, .. } => (key, emission.verified),
        FlowchartEffectOutcome::Residual { key } => (key, false),
        FlowchartEffectOutcome::Cleared { .. } => unreachable!("clear was handled above"),
    };
    match key {
        FamilyThemeMechanismKey::Rule { index, .. } => {
            if applied {
                state.node.applied_rules.insert(*index);
                state
                    .node
                    .applied_rule_capabilities
                    .entry(*index)
                    .or_default()
                    .extend([ThemeCapability::Shadow, ThemeCapability::SvgFilter]);
            } else {
                state
                    .node
                    .residual_rules
                    .entry(*index)
                    .or_insert(FamilyThemeResidualReason::UnsupportedEffect);
            }
        }
        FamilyThemeMechanismKey::EffectBinding { .. } => {
            let observation = state.node_effect_bindings.entry(key.clone()).or_default();
            if applied {
                observation.observe_applied();
            } else {
                observation.observe_residual(FamilyThemeResidualReason::UnsupportedEffect);
            }
        }
        _ => unreachable!("node effects belong to a rule or effect binding"),
    }
}

fn record_ordinal_palette_outcome(
    observation: &mut FlowchartMechanismObservation,
    outcome: Option<&FlowchartOrdinalPaletteOutcome>,
    precedence: FlowchartFacetPrecedence,
    channel_emitted: bool,
) {
    if precedence.overrides_theme() {
        return;
    }
    match outcome {
        Some(FlowchartOrdinalPaletteOutcome::Candidate { .. }) if channel_emitted => {
            observation.observe_applied();
        }
        Some(FlowchartOrdinalPaletteOutcome::Candidate { .. }) => {
            observation.observe_residual(FamilyThemeResidualReason::UnsupportedPaint);
        }
        Some(FlowchartOrdinalPaletteOutcome::Residual { reason }) => {
            observation.observe_residual(*reason);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette, OrdinalSelector,
        TextStylePatch, ThemeColorValue, ThemeGeometryPatch, ThemeRule, ThemeRuleSet,
        ThemeStrokePatch, ThemeStylePatch,
    };

    fn node_effect_theme(
        effect: Specified<String>,
        supported: bool,
        fill: bool,
    ) -> ResolvedDiagramTheme {
        use crate::diagram_theme::{
            DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
            ThemeEffectPatch,
        };
        let graph = |id: &str| {
            EffectGraph::new(
                id,
                [if supported {
                    EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: 2.0,
                        offset_y: 3.0,
                        blur_radius: 4.0,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#00f2ff").unwrap(),
                    }
                } else {
                    EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 2.0,
                    }
                }],
            )
            .unwrap()
        };
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_effects(
                        DiagramEffectSet::default()
                            .with_graph(graph("binding"))
                            .unwrap()
                            .with_graph(graph("rule"))
                            .unwrap()
                            .with_binding(EffectBinding::new(ThemeTarget::Node, "binding").unwrap())
                            .unwrap(),
                    )
                    .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch {
                            effects: ThemeEffectPatch { effect },
                            ..if fill {
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#112233").unwrap())
                            } else {
                                ThemeStylePatch::default()
                            }
                        },
                    ))),
            )
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn node_effect_binding_key() -> FamilyThemeMechanismKey {
        FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::Node,
            effect_id: "binding".to_owned(),
        }
    }

    fn node_effect_rule_key() -> FamilyThemeMechanismKey {
        FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Node,
        }
    }

    #[test]
    fn node_effect_winning_rule_and_clear_suppress_the_global_binding() {
        for (specified, expected) in [
            (Specified::Value("rule".to_owned()), Some("rule")),
            (Specified::Clear, None),
        ] {
            let theme = node_effect_theme(specified, true, false);
            let meter = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
            assert_eq!(style.effect().map(|effect| effect.id()), expected);
            let recorder = FlowchartThemeEvidenceRecorder::default();
            let mut observed = emission(true, false, false, false, false);
            observed.effect = FlowchartThemeFacetEmission::new(no_override(), true);
            recorder
                .record_node_emission(&style, observed, &[], &meter)
                .unwrap();
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(evidence.applied().contains(&node_effect_rule_key()));
            assert!(
                evidence
                    .not_applicable_mechanisms()
                    .contains(&node_effect_binding_key())
            );
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence
                    .applied_capabilities()
                    .contains(&ThemeCapability::Shadow),
                expected.is_some()
            );
        }
    }

    #[test]
    fn node_effect_global_binding_requires_every_applicable_terminal() {
        for verified in [true, false] {
            let theme = node_effect_theme(Specified::Unspecified, true, true);
            let meter = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
            assert_eq!(style.effect().unwrap().id(), "binding");
            let recorder = FlowchartThemeEvidenceRecorder::default();
            for verified in [true, verified] {
                let mut observed = emission(true, false, false, false, false);
                observed.effect = FlowchartThemeFacetEmission::new(no_override(), verified);
                recorder
                    .record_node_emission(&style, observed, &[], &meter)
                    .unwrap();
            }
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(evidence.applied().contains(&node_effect_rule_key()));
            assert_eq!(
                evidence.applied().contains(&node_effect_binding_key()),
                verified
            );
            assert_eq!(evidence.residuals().is_empty(), verified);
            if !verified {
                assert_eq!(
                    evidence.residuals()[0].reason(),
                    FamilyThemeResidualReason::UnsupportedEffect
                );
            }
        }
    }

    #[test]
    fn node_effect_unsupported_graph_or_partial_rule_emission_remains_residual() {
        for supported in [false, true] {
            let theme = node_effect_theme(Specified::Value("rule".to_owned()), supported, true);
            let meter = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
            assert_eq!(style.effect().is_some(), supported);
            let recorder = FlowchartThemeEvidenceRecorder::default();
            let emissions = if supported {
                &[true, false][..]
            } else {
                &[true][..]
            };
            for &verified in emissions {
                let mut observed = emission(true, false, false, false, false);
                observed.effect = FlowchartThemeFacetEmission::new(no_override(), verified);
                recorder
                    .record_node_emission(&style, observed, &[], &meter)
                    .unwrap();
            }
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(!evidence.applied().contains(&node_effect_rule_key()));
            assert_eq!(
                evidence.residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedEffect
            );
            assert!(
                evidence
                    .not_applicable_mechanisms()
                    .contains(&node_effect_binding_key())
            );
        }
    }

    #[test]
    fn node_effect_unsupported_binding_cannot_be_promoted_by_a_writer_receipt() {
        let theme = node_effect_theme(Specified::Unspecified, false, true);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
        assert!(style.effect().is_none());
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let mut observed = emission(true, false, false, false, false);
        observed.effect = FlowchartThemeFacetEmission::new(no_override(), true);
        recorder
            .record_node_emission(&style, observed, &[], &meter)
            .unwrap();
        let (evidence, _) = recorder.finish(Some(&theme));
        assert!(evidence.applied().contains(&node_effect_rule_key()));
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(evidence.residuals()[0].key(), &node_effect_binding_key());
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedEffect
        );
    }

    #[test]
    fn node_effect_source_override_preserves_the_sibling_fill() {
        for specified in [
            Specified::Unspecified,
            Specified::Value("rule".to_owned()),
            Specified::Clear,
        ] {
            let theme = node_effect_theme(specified, true, true);
            let meter = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
            let recorder = FlowchartThemeEvidenceRecorder::default();
            let mut observed = emission(true, false, false, false, false);
            observed.effect = FlowchartThemeFacetEmission::new(
                FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Unverified, false),
                false,
            );
            recorder
                .record_node_emission(&style, observed, &[], &meter)
                .unwrap();
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(evidence.applied().contains(&node_effect_rule_key()));
            assert!(
                evidence
                    .not_applicable_mechanisms()
                    .contains(&node_effect_binding_key())
            );
            assert!(evidence.residuals().is_empty());
            assert!(
                !evidence
                    .applied_capabilities()
                    .contains(&ThemeCapability::Shadow)
            );
        }
    }

    #[test]
    fn node_effect_without_node_occurrences_is_not_applicable() {
        let theme = node_effect_theme(Specified::Value("rule".to_owned()), true, true);
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let (evidence, _) = recorder.finish(Some(&theme));
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&node_effect_binding_key())
        );
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&node_effect_rule_key())
        );
    }

    fn static_provenance_theme(rule_count: usize) -> ResolvedDiagramTheme {
        let mut rules = ThemeRuleSet::default();
        for _ in 0..rule_count {
            rules = rules.with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#ef4444").expect("valid static fill")),
            ));
        }
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile static provenance workload")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    #[test]
    fn static_provenance_workload_charges_512_rules_once_across_1000_nodes() {
        let theme = static_provenance_theme(512);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let mut first = theme.style(ThemeTarget::Node, ThemeVariant::Default, None);
        let shared = first.take_matched_rules();
        assert_eq!(shared.static_blocks().len(), 1);
        for ordinal in 1..=1000 {
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(ordinal), &meter)
                .expect("resolve static node style");
            assert_eq!(style.matched_rules.static_blocks().len(), 1);
            assert!(Arc::ptr_eq(
                &shared.static_blocks()[0],
                &style.matched_rules.static_blocks()[0]
            ));
            assert!(style.matched_rules.dynamic_indices().is_empty());
            recorder
                .record_node_emission(
                    &style,
                    emission(true, false, false, false, false),
                    &[],
                    &meter,
                )
                .expect("record metered theme emission");
        }
        let state = recorder.state.lock().expect("read provenance state");
        assert_eq!(state.node.matched_rules.len(), 512);
        assert_eq!(
            meter.used(),
            512,
            "static provenance traversal must be metered"
        );
    }

    #[test]
    fn provenance_budget_rejection_preserves_all_recorder_state() {
        let theme = static_provenance_theme(512);
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 511)
            .expect("set provenance ceiling");
        let meter = OperationWorkMeter::new(policy);
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("shared static resolution needs no index traversal");
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let before = format!("{:?}", recorder.state.lock().unwrap());
        assert!(
            recorder
                .record_node_emission(
                    &style,
                    emission(true, false, false, false, false),
                    &[],
                    &meter
                )
                .is_err()
        );
        assert_eq!(meter.used(), 0);
        assert_eq!(format!("{:?}", recorder.state.lock().unwrap()), before);
    }

    #[test]
    fn node_and_label_provenance_are_charged_before_either_is_recorded() {
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            ));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART);
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let mut observed = emission(true, false, false, false, false);
        observed.label_fill = Some(FlowchartThemeFacetEmission::new(no_override(), true));
        let before = format!("{:?}", recorder.state.lock().unwrap());
        assert!(
            recorder
                .record_node_emission(&style, observed, &[], &meter)
                .is_err()
        );
        assert_eq!(meter.used(), 0);
        assert_eq!(format!("{:?}", recorder.state.lock().unwrap()), before);

        let meter = OperationWorkMeter::new(policy);
        recorder
            .record_node_emission(
                &style,
                emission(true, false, false, false, false),
                &[],
                &meter,
            )
            .unwrap();
        assert_eq!(meter.used(), 1);
        let state = recorder.state.lock().unwrap();
        assert!(state.node.emitted);
        assert!(!state.node_label.emitted);
        assert!(state.node_label.matched_rules.is_empty());
        assert!(state.node_label.seen_static_blocks.is_empty());
    }

    #[test]
    fn repeated_dynamic_provenance_remains_metered_after_static_reuse() {
        let patch = || ThemeStylePatch::default().with_fill(CanvasPaint::Transparent);
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, patch()))
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_ordinal(OrdinalSelector::exact(99).unwrap()),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let recorder = FlowchartThemeEvidenceRecorder::default();
        for expected in [4, 7] {
            let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
            assert_eq!(style.matched_rules.dynamic_indices(), &BTreeSet::from([1]));
            recorder
                .record_node_emission(
                    &style,
                    emission(true, false, false, false, false),
                    &[],
                    &meter,
                )
                .unwrap();
            assert_eq!(meter.used(), expected);
        }
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter).unwrap();
        assert!(style.matched_rules.dynamic_indices().is_empty());
        recorder
            .record_node_emission(
                &style,
                emission(true, false, false, false, false),
                &[],
                &meter,
            )
            .unwrap();
        assert_eq!(meter.used(), 9);
        let before = format!("{:?}", recorder.state.lock().unwrap());
        let limited = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        );
        limited.charge(1).unwrap();
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter).unwrap();
        assert!(
            recorder
                .record_node_emission(
                    &style,
                    emission(true, false, false, false, false),
                    &[],
                    &limited
                )
                .is_err()
        );
        assert_eq!(limited.used(), 1);
        assert_eq!(format!("{:?}", recorder.state.lock().unwrap()), before);
    }

    #[test]
    fn provenance_accounts_for_each_variant_static_block_once() {
        let patch = || ThemeStylePatch::default().with_fill(CanvasPaint::Transparent);
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, patch()))
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch()).with_variant(ThemeVariant::Active),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch()).with_variant(ThemeVariant::Selected),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let recorder = FlowchartThemeEvidenceRecorder::default();
        for (variant, expected) in [
            (ThemeVariant::Active, 2),
            (ThemeVariant::Selected, 3),
            (ThemeVariant::Active, 3),
        ] {
            let mut resolved = theme.style(ThemeTarget::Node, variant, None);
            let style = FlowchartNodeThemeStyle {
                matched_rules: resolved.take_matched_rules(),
                ..Default::default()
            };
            recorder
                .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
                .unwrap();
            assert_eq!(meter.used(), expected);
        }
        let state = recorder.state.lock().unwrap();
        assert_eq!(state.node.matched_rules, BTreeSet::from([0, 1, 2]));
        assert_eq!(state.node.seen_static_blocks.len(), 3);
    }

    #[test]
    #[ignore = "manual static provenance workload benchmark"]
    fn static_provenance_workload_benchmark() {
        for rule_count in [1, 64, 512] {
            let theme = static_provenance_theme(rule_count);
            let mut samples = Vec::new();
            let mut work_units = 0;
            for _ in 0..5 {
                let meter = OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                );
                let recorder = FlowchartThemeEvidenceRecorder::default();
                let start = std::time::Instant::now();
                for ordinal in 1..=1000 {
                    let style =
                        FlowchartNodeThemeStyle::resolve(Some(&theme), Some(ordinal), &meter)
                            .expect("resolve benchmark node style");
                    recorder
                        .record_node_emission(
                            std::hint::black_box(&style),
                            emission(true, false, false, false, false),
                            &[],
                            &meter,
                        )
                        .expect("record metered theme emission");
                }
                samples.push(start.elapsed());
                work_units = meter.used();
                assert_eq!(
                    recorder
                        .state
                        .lock()
                        .expect("read benchmark state")
                        .node
                        .matched_rules
                        .len(),
                    rule_count,
                );
            }
            samples.sort();
            println!(
                "static provenance: rules={rule_count}, nodes=1000, median={:?}, work_units={work_units}",
                samples[2],
            );
        }
    }

    fn no_override() -> FlowchartFacetPrecedence {
        FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, false)
    }

    fn emission(
        fill: bool,
        stroke: bool,
        stroke_width: bool,
        stroke_dasharray: bool,
        radius: bool,
    ) -> FlowchartNodeThemeEmission {
        FlowchartNodeThemeEmission {
            fill: FlowchartThemeFacetEmission::new(no_override(), fill),
            stroke: FlowchartThemeFacetEmission::new(no_override(), stroke),
            stroke_width: FlowchartThemeFacetEmission::new(no_override(), stroke_width),
            stroke_dasharray: FlowchartThemeFacetEmission::new(no_override(), stroke_dasharray),
            radius: FlowchartThemeFacetEmission::new(no_override(), radius),
            effect: FlowchartThemeFacetEmission::new(no_override(), false),
            label_fill: None,
            font_stack: None,
            font_size: None,
        }
    }

    fn resolved_theme(target: ThemeTarget) -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            target,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_edge_stroke_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart Edge stroke theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_edge_dasharray_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke_dasharray([4.0, 2.0])
                                .expect("valid Flowchart Edge dasharray"),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart Edge dasharray theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_edge_stroke_width_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke_width(2.5)
                                .expect("valid Flowchart Edge stroke width"),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart Edge stroke-width theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn edge_emission(
        precedence: FlowchartFacetPrecedence,
        verified: bool,
    ) -> FlowchartEdgeThemeEmission {
        FlowchartEdgeThemeEmission {
            stroke: FlowchartThemeFacetEmission::new(precedence, verified),
            stroke_width: FlowchartThemeFacetEmission::absent(),
            stroke_dasharray: FlowchartThemeFacetEmission::absent(),
        }
    }

    fn edge_dasharray_emission(
        precedence: FlowchartFacetPrecedence,
        verified: bool,
    ) -> FlowchartEdgeThemeEmission {
        FlowchartEdgeThemeEmission {
            stroke: FlowchartThemeFacetEmission::absent(),
            stroke_width: FlowchartThemeFacetEmission::absent(),
            stroke_dasharray: FlowchartThemeFacetEmission::new(precedence, verified),
        }
    }

    fn edge_stroke_width_emission(
        precedence: FlowchartFacetPrecedence,
        verified: bool,
    ) -> FlowchartEdgeThemeEmission {
        FlowchartEdgeThemeEmission {
            stroke: FlowchartThemeFacetEmission::absent(),
            stroke_width: FlowchartThemeFacetEmission::new(precedence, verified),
            stroke_dasharray: FlowchartThemeFacetEmission::absent(),
        }
    }

    fn resolved_cluster_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Cluster,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap())
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart Cluster theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn cluster_emission(fill: bool, stroke: bool) -> FlowchartClusterThemeEmission {
        FlowchartClusterThemeEmission {
            fill: FlowchartThemeFacetEmission::new(no_override(), fill),
            stroke: FlowchartThemeFacetEmission::new(no_override(), stroke),
        }
    }

    #[test]
    fn cluster_paint_requires_complete_terminal_emission() {
        let theme = resolved_cluster_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartClusterThemeStyle::resolve(Some(&theme), None, &meter)
            .expect("resolve Flowchart Cluster theme");

        assert_eq!(style.fill_value(no_override(), true), Some("#ef4444"));
        assert_eq!(style.stroke_value(no_override(), true), Some("#2563eb"));

        let empty = FlowchartThemeEvidenceRecorder::default();
        let (empty_evidence, source_residuals) = empty.finish(Some(&theme));
        assert!(source_residuals.is_empty());
        assert!(empty_evidence.applied().is_empty());
        assert_eq!(
            empty_evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Cluster,
            }]
        );

        let incomplete = FlowchartThemeEvidenceRecorder::default();
        incomplete
            .record_cluster_emission(&style, cluster_emission(true, false), &[], &meter)
            .expect("record metered theme emission");
        let (incomplete_evidence, source_residuals) = incomplete.finish(Some(&theme));
        assert!(source_residuals.is_empty());
        assert!(incomplete_evidence.applied().is_empty());
        assert_eq!(incomplete_evidence.residuals().len(), 1);
        assert_eq!(
            incomplete_evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );

        let complete = FlowchartThemeEvidenceRecorder::default();
        complete
            .record_cluster_emission(&style, cluster_emission(true, true), &[], &meter)
            .expect("record metered theme emission");
        let (complete_evidence, source_residuals) = complete.finish(Some(&theme));
        assert!(source_residuals.is_empty());
        assert_eq!(
            complete_evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Cluster,
            }]
        );
        assert!(complete_evidence.residuals().is_empty());
        assert_eq!(
            complete_evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::SolidPaint])
        );
    }

    #[test]
    fn inline_style_preserves_existing_declarations_and_emitted_facets() {
        let style = FlowchartNodeThemeStyle {
            fill: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#ef4444".to_string(),
            }),
            ordinal_palette_fill: None,
            stroke: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#2563eb".to_string(),
            }),
            stroke_width: Some(FlowchartScalarOutcome::Candidate {
                rule_index: 0,
                value: 2.5,
            }),
            stroke_dasharray: Some(FlowchartDasharrayOutcome::Candidate {
                rule_index: 0,
                value: "4 2".to_string(),
            }),
            radius: None,
            effect: None,
            matched_rules: MatchedThemeRules::from_indices([0]),
            residual_rules: BTreeMap::new(),
            incomplete_rules: BTreeSet::new(),
            label: FlowchartLabelThemeStyle::default(),
        };
        let mut inline = "opacity:0.5".to_string();

        style.append_inline_style(
            &mut inline,
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            no_override(),
            no_override(),
            no_override(),
            true,
            true,
            true,
            true,
        );

        assert_eq!(
            inline,
            "opacity:0.5;stroke:#2563eb !important;stroke-width:2.5px !important;stroke-dasharray:4 2 !important"
        );
    }

    fn resolved_stroke_width_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_stroke_width(2.5)
                                .expect("valid stroke width"),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart stroke-width theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_stroke_dasharray_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_stroke_dasharray([4.0, 2.0])
                                .expect("valid stroke dasharray"),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart stroke-dasharray theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_radius_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch {
                                geometry: ThemeGeometryPatch {
                                    radius: Specified::Value(8.0),
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart radius theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_node_label_font_stack_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch {
                                typography: TextStylePatch {
                                    font_stack: Specified::Value(
                                        FontStack::single("Excalifont")
                                            .expect("valid fixture font stack"),
                                    ),
                                    ..TextStylePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart NodeLabel font-stack theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_node_label_font_size_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch {
                                typography: TextStylePatch {
                                    font_size_px: Specified::Value(26.0),
                                    ..TextStylePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart NodeLabel font-size theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_edge_label_typography_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::EdgeLabel,
                                ThemeStylePatch {
                                    typography: TextStylePatch {
                                        font_stack: Specified::Value(
                                            FontStack::single("Excalifont")
                                                .expect("valid fixture font stack"),
                                        ),
                                        ..TextStylePatch::default()
                                    },
                                    ..ThemeStylePatch::default()
                                },
                            )
                            .for_family(DiagramFamilyId::FLOWCHART),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::EdgeLabel,
                                ThemeStylePatch {
                                    typography: TextStylePatch {
                                        font_size_px: Specified::Value(26.0),
                                        ..TextStylePatch::default()
                                    },
                                    ..ThemeStylePatch::default()
                                },
                            )
                            .for_family(DiagramFamilyId::FLOWCHART),
                        ),
                ),
            )
            .expect("compile Flowchart EdgeLabel typography theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn resolved_edge_label_padding_theme() -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::EdgeLabel,
                            ThemeStylePatch::default().with_padding(
                                crate::diagram_theme::InsetsPx {
                                    top: 3.0,
                                    right: 7.0,
                                    bottom: 5.0,
                                    left: 11.0,
                                },
                            ),
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile Flowchart EdgeLabel padding theme")
            .resolve(DiagramFamilyId::FLOWCHART)
    }

    fn font_stack_emission(
        precedence: FlowchartFacetPrecedence,
        verified: bool,
    ) -> FlowchartNodeThemeEmission {
        let mut emission = FlowchartNodeThemeEmission::none();
        emission.font_stack = Some(FlowchartThemeFacetEmission::new(precedence, verified));
        emission
    }

    fn font_size_emission(
        precedence: FlowchartFacetPrecedence,
        verified: bool,
    ) -> FlowchartNodeThemeEmission {
        let mut emission = FlowchartNodeThemeEmission::none();
        emission.font_size = Some(FlowchartThemeFacetEmission::new(precedence, verified));
        emission
    }

    fn edge_label_emission(
        font_stack: Option<(FlowchartFacetPrecedence, bool)>,
        font_size: Option<(FlowchartFacetPrecedence, bool)>,
    ) -> FlowchartEdgeLabelThemeEmission {
        FlowchartEdgeLabelThemeEmission {
            font_stack: font_stack.map(|(precedence, verified)| {
                FlowchartThemeFacetEmission::new(precedence, verified)
            }),
            font_size: font_size.map(|(precedence, verified)| {
                FlowchartThemeFacetEmission::new(precedence, verified)
            }),
            padding: None,
        }
    }

    #[test]
    fn verified_edge_label_typography_is_applied_per_rule() {
        let theme = resolved_edge_label_typography_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart EdgeLabel typography");
        let precedence = no_override();

        assert!(style.font_stack_selected(precedence));
        assert!(style.font_size_selected(precedence));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_label_emission(
                &style,
                edge_label_emission(Some((precedence, true)), Some((precedence, true))),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::EdgeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::EdgeLabel,
                },
            ]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::Typography])
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn verified_edge_label_padding_is_applied_as_content_padding() {
        let theme = resolved_edge_label_padding_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart EdgeLabel padding");
        let precedence = no_override();
        let padding = style.edge_label_padding();

        assert!(style.padding_selected(precedence));
        assert_eq!(padding.padded_size(40.0, 20.0), (58.0, 28.0));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_label_emission(
                &style,
                FlowchartEdgeLabelThemeEmission {
                    font_stack: None,
                    font_size: None,
                    padding: Some(FlowchartThemeFacetEmission::new(precedence, true)),
                },
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::EdgeLabel,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::ContentPadding])
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn edge_label_source_and_config_shadow_only_their_own_typography_facets() {
        let theme = resolved_edge_label_typography_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart EdgeLabel typography");

        for (font_stack, font_size, applied_index, shadowed_index) in [
            (
                (no_override(), true),
                (
                    FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
                    false,
                ),
                0,
                1,
            ),
            (
                (
                    FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
                    false,
                ),
                (no_override(), true),
                1,
                0,
            ),
        ] {
            let recorder = FlowchartThemeEvidenceRecorder::default();
            recorder
                .record_edge_label_emission(
                    &style,
                    edge_label_emission(Some(font_stack), Some(font_size)),
                    &[],
                    &meter,
                )
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert_eq!(
                evidence.applied(),
                &[FamilyThemeMechanismKey::Rule {
                    index: applied_index,
                    target: ThemeTarget::EdgeLabel,
                }]
            );
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: shadowed_index,
                    target: ThemeTarget::EdgeLabel,
                }]
            );
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn one_unverified_edge_label_cannot_be_hidden_by_an_applied_label() {
        let theme = resolved_edge_label_typography_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart EdgeLabel typography");
        let font_size_shadow =
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false);
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_edge_label_emission(
                &style,
                edge_label_emission(Some((no_override(), true)), Some((font_size_shadow, false))),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        recorder
            .record_edge_label_emission(
                &style,
                edge_label_emission(
                    Some((no_override(), false)),
                    Some((font_size_shadow, false)),
                ),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::EdgeLabel,
            }]
        );
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::EdgeLabel,
            }
        );
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }

    #[test]
    fn edge_label_typography_without_an_applicable_label_is_not_applicable() {
        let theme = resolved_edge_label_typography_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart EdgeLabel typography");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_edge_label_emission(&style, edge_label_emission(None, None), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::EdgeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::EdgeLabel,
                },
            ]
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn verified_node_label_font_size_is_applied_with_typography_capability() {
        let theme = resolved_node_label_font_size_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font size");
        let precedence = no_override();

        assert!(style.font_size_selected(precedence));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(&style, font_size_emission(precedence, true), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::Typography])
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_node_label_font_size_is_a_typography_residual() {
        let theme = resolved_node_label_font_size_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font size");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(
                &style,
                font_size_emission(no_override(), false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }

    #[test]
    fn source_or_mermaid_config_font_size_shadows_only_the_typed_font_size_rule() {
        let theme = resolved_node_label_font_size_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font size");

        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert!(!style.font_size_selected(precedence));

            let recorder = FlowchartThemeEvidenceRecorder::default();
            recorder
                .record_node_emission(&style, font_size_emission(precedence, false), &[], &meter)
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                }]
            );
        }
    }

    #[test]
    fn verified_node_label_font_stack_is_applied_with_typography_capability() {
        let theme = resolved_node_label_font_stack_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font stack");
        let precedence = no_override();

        assert!(style.font_stack_selected(precedence));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(&style, font_stack_emission(precedence, true), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::Typography])
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_node_label_font_stack_is_a_typography_residual() {
        let theme = resolved_node_label_font_stack_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font stack");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(
                &style,
                font_stack_emission(no_override(), false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }

    #[test]
    fn source_or_mermaid_config_font_stack_shadows_the_typed_rule() {
        let theme = resolved_node_label_font_stack_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font stack");

        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert!(!style.font_stack_selected(precedence));

            let recorder = FlowchartThemeEvidenceRecorder::default();
            recorder
                .record_node_emission(&style, font_stack_emission(precedence, false), &[], &meter)
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                }]
            );
        }
    }

    #[test]
    fn node_without_an_emitted_label_makes_the_font_stack_not_applicable() {
        let theme = resolved_node_label_font_stack_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart NodeLabel font stack");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }]
        );
    }

    #[test]
    fn one_unverified_label_makes_the_font_stack_rule_residual() {
        let theme = resolved_node_label_font_stack_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let first = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve verified Flowchart NodeLabel font stack");
        let second = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter)
            .expect("resolve unverified Flowchart NodeLabel font stack");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(
                &first,
                font_stack_emission(no_override(), true),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        recorder
            .record_node_emission(
                &second,
                font_stack_emission(no_override(), false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }

    #[test]
    fn verified_node_stroke_width_is_applied() {
        let theme = resolved_stroke_width_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke width");
        let no_override = no_override();

        assert_eq!(style.stroke_width_value(no_override, true), Some(2.5));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &style,
                emission(false, false, true, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_node_stroke_width_is_a_geometry_residual() {
        let theme = resolved_stroke_width_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke width");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn source_or_mermaid_config_stroke_width_shadows_the_typed_rule() {
        let theme = resolved_stroke_width_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke width");

        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert_eq!(style.stroke_width_value(precedence, true), None);

            let recorder = FlowchartThemeEvidenceRecorder::default();
            let mut observed = FlowchartNodeThemeEmission::none();
            observed.stroke_width = FlowchartThemeFacetEmission::new(precedence, false);
            recorder
                .record_node_emission(&style, observed, &[], &meter)
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
        }
    }

    #[test]
    fn verified_node_stroke_dasharray_is_applied_with_dash_capability() {
        let theme = resolved_stroke_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke dasharray");
        let no_override = no_override();

        assert_eq!(style.stroke_dasharray_value(no_override, true), Some("4 2"));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &style,
                emission(false, false, false, true, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::DashStyling])
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_node_stroke_dasharray_is_a_geometry_residual() {
        let theme = resolved_stroke_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke dasharray");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn one_unverified_node_makes_the_stroke_dasharray_rule_residual() {
        let theme = resolved_stroke_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let first = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve verified Flowchart stroke dasharray");
        let second = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter)
            .expect("resolve unverified Flowchart stroke dasharray");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(
                &first,
                emission(false, false, false, true, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        recorder
            .record_node_emission(
                &second,
                emission(false, false, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn source_stroke_dasharray_shadows_the_typed_rule() {
        let theme = resolved_stroke_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart stroke dasharray");
        let precedence = FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false);

        assert_eq!(style.stroke_dasharray_value(precedence, true), None);

        let recorder = FlowchartThemeEvidenceRecorder::default();
        let mut observed = FlowchartNodeThemeEmission::none();
        observed.stroke_dasharray = FlowchartThemeFacetEmission::new(precedence, false);
        recorder
            .record_node_emission(&style, observed, &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn verified_node_radius_is_applied_with_rounded_geometry_capability() {
        let theme = resolved_radius_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart radius");
        let no_override = no_override();

        assert_eq!(style.radius_value(no_override, true), Some(8.0));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &style,
                emission(false, false, false, false, true),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::RoundedGeometry])
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn unverified_node_radius_is_a_geometry_residual() {
        let theme = resolved_radius_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart radius");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn source_or_mermaid_config_radius_shadows_the_typed_rule() {
        let theme = resolved_radius_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve Flowchart radius");
        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert_eq!(style.radius_value(precedence, true), None);

            let recorder = FlowchartThemeEvidenceRecorder::default();
            let mut observed = FlowchartNodeThemeEmission::none();
            observed.radius = FlowchartThemeFacetEmission::new(precedence, false);
            recorder
                .record_node_emission(&style, observed, &[], &meter)
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
        }
    }

    #[test]
    fn unemitted_node_paint_is_residual_instead_of_applied() {
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let theme = resolved_theme(ThemeTarget::Node);
        let style = FlowchartNodeThemeStyle {
            fill: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#ef4444".to_string(),
            }),
            ordinal_palette_fill: None,
            stroke: None,
            stroke_width: None,
            stroke_dasharray: None,
            radius: None,
            effect: None,
            matched_rules: MatchedThemeRules::from_indices([0]),
            residual_rules: BTreeMap::new(),
            incomplete_rules: BTreeSet::new(),
            label: FlowchartLabelThemeStyle::default(),
        };
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn unobserved_edge_rule_is_not_applicable() {
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let theme = resolved_edge_stroke_theme();
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &FlowchartNodeThemeStyle::default(),
                emission(true, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
    }

    #[test]
    fn verified_edge_stroke_is_applied_with_solid_paint_capability() {
        let theme = resolved_edge_stroke_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge stroke");
        let precedence = no_override();

        assert_eq!(style.stroke_value(precedence, true), Some("#ef4444"));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_emission(&style, edge_emission(precedence, true), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::SolidPaint])
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn verified_edge_dasharray_is_applied_with_dash_styling_capability() {
        let theme = resolved_edge_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge dasharray");
        let precedence = no_override();

        assert_eq!(style.stroke_dasharray_value(precedence, true), Some("4 2"));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_emission(
                &style,
                edge_dasharray_emission(precedence, true),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::DashStyling])
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn verified_edge_stroke_width_is_applied_with_border_styling_capability() {
        let theme = resolved_edge_stroke_width_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge stroke width");
        let precedence = no_override();

        assert_eq!(style.stroke_width_value(precedence, true), Some(2.5));

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_emission(
                &style,
                edge_stroke_width_emission(precedence, true),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([ThemeCapability::BorderStyling])
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn source_edge_dasharray_makes_typed_rule_not_applicable() {
        let theme = resolved_edge_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge dasharray");
        let precedence = FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false);

        assert_eq!(style.stroke_dasharray_value(precedence, true), None);

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_edge_emission(
                &style,
                edge_dasharray_emission(precedence, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Edge,
            }]
        );
    }

    #[test]
    fn one_unverified_edge_path_keeps_dasharray_rule_fail_closed() {
        let theme = resolved_edge_dasharray_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge dasharray");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_edge_emission(
                &style,
                edge_dasharray_emission(no_override(), false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        recorder
            .record_edge_emission(
                &style,
                edge_dasharray_emission(no_override(), true),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn cleared_edge_dasharray_is_an_explicit_geometry_residual() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch {
                                stroke: ThemeStrokePatch {
                                    dasharray: Specified::Clear,
                                    ..ThemeStrokePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile cleared Flowchart Edge dasharray theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve cleared Flowchart Edge dasharray");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        assert_eq!(style.stroke_dasharray_value(no_override(), true), None);
        recorder
            .record_edge_emission(
                &style,
                edge_dasharray_emission(no_override(), false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry
        );
    }

    #[test]
    fn source_or_mermaid_config_edge_stroke_makes_typed_rule_not_applicable() {
        let theme = resolved_edge_stroke_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge stroke");

        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Unverified, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert_eq!(style.stroke_value(precedence, true), None);

            let recorder = FlowchartThemeEvidenceRecorder::default();
            recorder
                .record_edge_emission(&style, edge_emission(precedence, false), &[], &meter)
                .expect("record metered theme emission");
            let (evidence, source_residuals) = recorder.finish(Some(&theme));

            assert!(source_residuals.is_empty());
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Edge,
                }]
            );
        }
    }

    #[test]
    fn one_unverified_edge_path_keeps_rule_residual_after_an_applied_path() {
        let theme = resolved_edge_stroke_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge stroke");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_edge_emission(&style, edge_emission(no_override(), false), &[], &meter)
            .expect("record metered theme emission");
        recorder
            .record_edge_emission(&style, edge_emission(no_override(), true), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn edge_without_a_terminal_path_fails_closed() {
        let theme = resolved_edge_stroke_theme();
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter)
            .expect("resolve Flowchart Edge stroke");
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder
            .record_edge_emission(&style, edge_emission(no_override(), false), &[], &meter)
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn unmatched_node_variant_is_not_applicable() {
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_variant(ThemeVariant::Active)
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile variant theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &FlowchartNodeThemeStyle::default(),
                emission(true, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn emitted_node_applies_the_typed_ordinal_palette() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#ef4444").unwrap(),
                            ThemeColorValue::parse("#2563eb").unwrap(),
                        ])
                        .unwrap(),
                    ),
                ),
            )
            .expect("compile ordinal Flowchart theme")
            .resolve(DiagramFamilyId::FLOWCHART);

        let (empty_evidence, source_residuals) =
            FlowchartThemeEvidenceRecorder::default().finish(Some(&theme));
        assert!(source_residuals.is_empty());
        assert!(empty_evidence.applied().is_empty());
        assert!(empty_evidence.residuals().is_empty());
        assert_eq!(
            empty_evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );

        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let first = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve first palette node");
        let second = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter)
            .expect("resolve second palette node");
        assert_eq!(
            first.fill_value(
                FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, false),
                true,
            ),
            Some("#ef4444")
        );
        assert_eq!(
            second.fill_value(
                FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, false),
                true,
            ),
            Some("#2563eb")
        );

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &first,
                emission(true, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.applied_capabilities(),
            BTreeSet::from([crate::diagram_theme::ThemeCapability::SolidPaint])
        );
    }

    #[test]
    fn source_fill_shadowing_makes_the_ordinal_palette_not_applicable() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([ThemeColorValue::parse("#ef4444").unwrap()]).unwrap(),
                ),
            ))
            .expect("compile ordinal Flowchart theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve shadowed palette node");
        let recorder = FlowchartThemeEvidenceRecorder::default();
        let mut observed = emission(false, true, false, false, false);
        observed.fill = FlowchartThemeFacetEmission::new(
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            false,
        );
        recorder
            .record_node_emission(&style, observed, &[], &meter)
            .expect("record metered theme emission");

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn one_unverified_node_makes_the_selected_ordinal_palette_residual() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#ef4444").unwrap(),
                            ThemeColorValue::parse("#2563eb").unwrap(),
                        ])
                        .unwrap(),
                    ),
                ),
            )
            .expect("compile ordinal Flowchart theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let first = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve verified palette node");
        let second = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter)
            .expect("resolve unverified palette node");
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &first,
                emission(true, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        recorder
            .record_node_emission(
                &second,
                emission(false, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn explicit_node_fill_rule_blocks_the_ordinal_palette_fallback() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#22c55e").unwrap()),
                            )
                            .for_family(DiagramFamilyId::FLOWCHART),
                        )
                        .with_ordinal_palette(
                            ThemeTarget::Node,
                            OrdinalPalette::new([ThemeColorValue::parse("#ef4444").unwrap()])
                                .unwrap(),
                        ),
                ),
            )
            .expect("compile rule and palette theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve explicit rule before palette");
        let no_override = no_override();
        assert_eq!(style.fill_value(no_override, true), Some("#22c55e"));
        assert!(style.ordinal_palette_fill.is_none());

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder
            .record_node_emission(
                &style,
                emission(true, true, false, false, false),
                &[],
                &meter,
            )
            .expect("record metered theme emission");
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert_eq!(
            evidence.applied(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn ordinal_node_rule_is_resolved_against_the_actual_one_based_node_ordinal() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::exact(2).unwrap())
                        .for_family(DiagramFamilyId::FLOWCHART),
                    ),
                ),
            )
            .expect("compile ordinal Flowchart rule")
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let first = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(1), &meter)
            .expect("resolve first node");
        assert!(first.matched_rules.is_empty());

        let second = FlowchartNodeThemeStyle::resolve(Some(&theme), Some(2), &meter)
            .expect("resolve second node");
        assert!(second.matched_rules.contains(&0));
        assert!(matches!(
            second.fill,
            Some(FlowchartPaintOutcome::Residual { rule_index: 0, .. })
        ));
    }

    #[test]
    fn edge_fill_missing_path_is_not_hidden_by_another_verified_path() {
        let theme = resolved_theme(ThemeTarget::Edge);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter).unwrap();
        assert_eq!(style.stroke_value(no_override(), true), Some("#ef4444"));
        assert_eq!(style.stroke_value(no_override(), false), None);
        for verified_paths in [vec![false], vec![false, true], vec![true, false]] {
            let recorder = FlowchartThemeEvidenceRecorder::default();
            for verified in verified_paths {
                recorder
                    .record_edge_emission(
                        &style,
                        edge_emission(no_override(), verified),
                        &[],
                        &meter,
                    )
                    .unwrap();
            }
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.residuals().len(), 1);
            assert_eq!(
                evidence.residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedPaint
            );
        }
    }

    #[test]
    fn edge_fill_uses_the_emitted_stroke_source_owner_including_unverified_source() {
        let theme = resolved_theme(ThemeTarget::Edge);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let style = FlowchartEdgeThemeStyle::resolve(Some(&theme), &meter).unwrap();
        for precedence in [
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Admitted, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Unverified, false),
            FlowchartFacetPrecedence::new(FlowchartSourceFacetStatus::Absent, true),
        ] {
            assert_eq!(style.stroke_value(precedence, true), None);
            let recorder = FlowchartThemeEvidenceRecorder::default();
            recorder
                .record_edge_emission(&style, edge_emission(precedence, false), &[], &meter)
                .unwrap();
            let (evidence, _) = recorder.finish(Some(&theme));
            assert!(evidence.applied().is_empty());
            assert!(evidence.residuals().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Edge
                }]
            );
        }
    }
}
