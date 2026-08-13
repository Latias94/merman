use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Mutex;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeRoute, FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedProperty,
    ResolvedStyleProperty, SourceStyleResidual, Specified, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkMeter, ResourceLimitExceeded};

#[derive(Debug)]
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

#[derive(Debug)]
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

impl FlowchartScalarOutcome {
    const fn value(&self) -> Option<f32> {
        match self {
            Self::Candidate { value, .. } => Some(*value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug)]
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartNodeThemeEmission {
    pub(crate) fill: FlowchartThemeFacetEmission,
    pub(crate) stroke: FlowchartThemeFacetEmission,
    pub(crate) stroke_width: FlowchartThemeFacetEmission,
    pub(crate) stroke_dasharray: FlowchartThemeFacetEmission,
    pub(crate) radius: FlowchartThemeFacetEmission,
    pub(crate) font_stack: Option<FlowchartThemeFacetEmission>,
    pub(crate) font_size: Option<FlowchartThemeFacetEmission>,
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
            font_stack: None,
            font_size: None,
        }
    }
}

#[derive(Debug, Default)]
struct FlowchartNodeLabelThemeStyle {
    font_stack: Option<FlowchartTypographyOutcome>,
    font_size: Option<FlowchartTypographyOutcome>,
    matched_rules: BTreeSet<usize>,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartNodeThemeStyle {
    fill: Option<FlowchartPaintOutcome>,
    ordinal_palette_fill: Option<FlowchartOrdinalPaletteOutcome>,
    stroke: Option<FlowchartPaintOutcome>,
    stroke_width: Option<FlowchartScalarOutcome>,
    stroke_dasharray: Option<FlowchartDasharrayOutcome>,
    radius: Option<FlowchartScalarOutcome>,
    matched_rules: BTreeSet<usize>,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
    label: FlowchartNodeLabelThemeStyle,
}

impl FlowchartNodeThemeStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, ResourceLimitExceeded> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            ordinal,
            work_meter,
        )?;
        let mut resolved = Self::default();
        let mut fill_rule_present = false;
        resolved.matched_rules.extend(style.matched_rule_indices());

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
                ResolvedStyleProperty::Effect => record_incomplete_facet(
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
                | ResolvedStyleProperty::Padding => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

        let label_style = theme.style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            ordinal,
            work_meter,
        )?;
        resolved
            .label
            .matched_rules
            .extend(label_style.matched_rule_indices());
        for (property, origin) in label_style.winner_rule_properties() {
            let rule_index = origin.rule_index();
            match property {
                ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontStack) => {
                    resolved.label.font_stack = resolve_typography(
                        theme,
                        rule_index,
                        &label_style.typography_resolution().patch().font_stack,
                        ThemeTypographyProperty::FontStack,
                    );
                }
                ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontSize) => {
                    resolved.label.font_size = resolve_typography(
                        theme,
                        rule_index,
                        &label_style.typography_resolution().patch().font_size_px,
                        ThemeTypographyProperty::FontSize,
                    );
                }
                ResolvedStyleProperty::Typography(property) => record_incomplete_label_facet(
                    theme,
                    &mut resolved.label,
                    rule_index,
                    FamilyThemeRuleFacet::Typography(property),
                ),
                ResolvedStyleProperty::Fill => record_incomplete_label_facet(
                    theme,
                    &mut resolved.label,
                    rule_index,
                    FamilyThemeRuleFacet::fill(label_style.fill_resolution().specified())
                        .expect("winning NodeLabel fill has a concrete facet"),
                ),
                ResolvedStyleProperty::Stroke => record_incomplete_label_facet(
                    theme,
                    &mut resolved.label,
                    rule_index,
                    FamilyThemeRuleFacet::stroke(label_style.stroke_resolution().specified())
                        .expect("winning NodeLabel stroke has a concrete facet"),
                ),
                ResolvedStyleProperty::Effect => record_incomplete_label_facet(
                    theme,
                    &mut resolved.label,
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
                | ResolvedStyleProperty::Padding => record_incomplete_label_facet(
                    theme,
                    &mut resolved.label,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

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
    resolved: &mut FlowchartNodeLabelThemeStyle,
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

fn unsupported_reason_for_facet(facet: FamilyThemeRuleFacet) -> FamilyThemeResidualReason {
    match facet {
        FamilyThemeRuleFacet::Typography(_) => FamilyThemeResidualReason::UnsupportedTypography,
        FamilyThemeRuleFacet::Effect => FamilyThemeResidualReason::UnsupportedEffect,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => {
            FamilyThemeResidualReason::UnsupportedPaint
        }
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding => FamilyThemeResidualReason::UnsupportedGeometry,
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
    applied_rules: BTreeSet<usize>,
    applied_rule_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
}

#[derive(Debug, Default)]
struct FlowchartThemeEvidenceState {
    node: FlowchartRuleEvidenceState,
    node_label: FlowchartRuleEvidenceState,
    node_ordinal_palette: FlowchartMechanismObservation,
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
    raw: String,
    property: Option<String>,
    owner_id: String,
    class_id: Option<String>,
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
            raw: residual.raw().to_string(),
            property: residual.property().map(str::to_string),
            owner_id: provenance.owner_id().to_string(),
            class_id: provenance.class_id().map(str::to_string),
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
    pub(crate) fn record_node_emission(
        &self,
        style: &FlowchartNodeThemeStyle,
        emission: FlowchartNodeThemeEmission,
        source_residuals: &[SourceStyleResidual],
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.node.emitted = true;
        state
            .node
            .matched_rules
            .extend(style.matched_rules.iter().copied());
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
        state
            .node
            .incomplete_rules
            .extend(style.incomplete_rules.iter().copied());
        if emission.font_stack.is_some() || emission.font_size.is_some() {
            state.node_label.emitted = true;
            state
                .node_label
                .matched_rules
                .extend(style.label.matched_rules.iter().copied());
            for (rule_index, reason) in &style.label.residual_rules {
                state
                    .node_label
                    .residual_rules
                    .entry(*rule_index)
                    .or_insert(*reason);
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
        let Some(theme) = theme else {
            return (evidence, source_residuals);
        };

        let mut rule_routes = BTreeMap::<(usize, ThemeTarget), Vec<FamilyThemeRoute>>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index, target, ..
                } => rule_routes
                    .entry((rule_index, target))
                    .or_default()
                    .push(route),
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
                } if state.node.emitted => {
                    if !matches!(
                        route.disposition(),
                        FamilyThemeDisposition::LegacyCompatibility
                    ) {
                        evidence.mark_residual(
                            theme.family_mechanism_key(route),
                            FamilyThemeResidualReason::UnsupportedEffect,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } => evidence.mark_not_applicable(theme.family_mechanism_key(route)),
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        for ((rule_index, target), _routes) in rule_routes {
            let target_state = match target {
                ThemeTarget::Node => &state.node,
                ThemeTarget::NodeLabel => &state.node_label,
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
        ThemeStylePatch,
    };

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
            matched_rules: BTreeSet::from([0]),
            residual_rules: BTreeMap::new(),
            incomplete_rules: BTreeSet::new(),
            label: FlowchartNodeLabelThemeStyle::default(),
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
        recorder.record_node_emission(&style, font_size_emission(precedence, true), &[]);
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

        recorder.record_node_emission(&style, font_size_emission(no_override(), false), &[]);
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
            recorder.record_node_emission(&style, font_size_emission(precedence, false), &[]);
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
        recorder.record_node_emission(&style, font_stack_emission(precedence, true), &[]);
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

        recorder.record_node_emission(&style, font_stack_emission(no_override(), false), &[]);
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
            recorder.record_node_emission(&style, font_stack_emission(precedence, false), &[]);
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

        recorder.record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[]);
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

        recorder.record_node_emission(&first, font_stack_emission(no_override(), true), &[]);
        recorder.record_node_emission(&second, font_stack_emission(no_override(), false), &[]);
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
        recorder.record_node_emission(&style, emission(false, false, true, false, false), &[]);
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

        recorder.record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[]);
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
            recorder.record_node_emission(&style, observed, &[]);
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
        recorder.record_node_emission(&style, emission(false, false, false, true, false), &[]);
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

        recorder.record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[]);
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

        recorder.record_node_emission(&first, emission(false, false, false, true, false), &[]);
        recorder.record_node_emission(&second, emission(false, false, false, false, false), &[]);
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
        recorder.record_node_emission(&style, observed, &[]);
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
        recorder.record_node_emission(&style, emission(false, false, false, false, true), &[]);
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

        recorder.record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[]);
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
            recorder.record_node_emission(&style, observed, &[]);
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
            matched_rules: BTreeSet::from([0]),
            residual_rules: BTreeMap::new(),
            incomplete_rules: BTreeSet::new(),
            label: FlowchartNodeLabelThemeStyle::default(),
        };
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder.record_node_emission(&style, FlowchartNodeThemeEmission::none(), &[]);
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
    fn unobserved_non_node_rule_remains_unaccounted() {
        let theme = resolved_theme(ThemeTarget::Edge);
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder.record_node_emission(
            &FlowchartNodeThemeStyle::default(),
            emission(true, true, false, false, false),
            &[],
        );

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn unmatched_node_variant_is_not_applicable() {
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
        recorder.record_node_emission(
            &FlowchartNodeThemeStyle::default(),
            emission(true, true, false, false, false),
            &[],
        );

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
        recorder.record_node_emission(&first, emission(true, true, false, false, false), &[]);

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
        recorder.record_node_emission(&style, observed, &[]);

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
        recorder.record_node_emission(&first, emission(true, true, false, false, false), &[]);
        recorder.record_node_emission(&second, emission(false, true, false, false, false), &[]);

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
        recorder.record_node_emission(&style, emission(true, true, false, false, false), &[]);
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
}
