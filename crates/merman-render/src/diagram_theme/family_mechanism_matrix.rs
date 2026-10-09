#[cfg(any(test, merman_internal_theme_acceptance))]
mod audit;
#[cfg(any(test, merman_internal_theme_acceptance))]
pub(crate) use audit::legacy_replacing_typed_routes;

use crate::DiagramFamilyId;
#[cfg(test)]
use crate::theme_route_cutover::ThemeRouteCutoverProjection;
#[cfg(test)]
use audit::*;
#[cfg(test)]
use merman_theme_contract::{
    ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1,
};

use super::canvas::CanvasPaint;
use super::resolved::ThemeTypographyProperty;
use super::semantic::{OrdinalSelector, ThemeRule, ThemeTarget, ThemeVariant};
use super::typography::{Specified, TextStyle};

/// Matches the per-string limit used by the core compatibility overlay.
///
/// Font stacks above this limit cannot enter the legacy configuration lane and
/// must remain unsupported so the matrix cannot claim an adaptation the bridge drops.
pub(super) const MAX_LEGACY_ASSIGNMENT_STRING_BYTES: usize = 4 * 1024;

/// Static owner of one family-local theme mechanism.
///
/// This classification routes work. It never claims that a concrete document applied a
/// mechanism; only the selected family consumer can produce runtime evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemeDisposition {
    TypedAdapter,
    LegacyCompatibility,
    Unsupported,
}

#[cfg(test)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct FamilyThemeSupportSummary {
    typed: bool,
    legacy: bool,
    unsupported: bool,
}

#[cfg(test)]
impl FamilyThemeSupportSummary {
    fn record(&mut self, disposition: FamilyThemeDisposition) {
        match disposition {
            FamilyThemeDisposition::TypedAdapter => self.typed = true,
            FamilyThemeDisposition::LegacyCompatibility => self.legacy = true,
            FamilyThemeDisposition::Unsupported => self.unsupported = true,
        }
    }

    pub(super) const fn has_typed(self) -> bool {
        self.typed
    }

    pub(super) const fn has_legacy(self) -> bool {
        self.legacy
    }

    pub(super) const fn has_unsupported(self) -> bool {
        self.unsupported
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemePaintKind {
    Clear,
    Transparent,
    Solid,
    LinearGradient,
    RadialGradient,
    Pattern,
}

impl FamilyThemePaintKind {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 6] = [
        Self::Clear,
        Self::Transparent,
        Self::Solid,
        Self::LinearGradient,
        Self::RadialGradient,
        Self::Pattern,
    ];

    fn from_specified(paint: &Specified<CanvasPaint>) -> Option<Self> {
        match paint {
            Specified::Unspecified => None,
            Specified::Clear => Some(Self::Clear),
            Specified::Value(CanvasPaint::Transparent) => Some(Self::Transparent),
            Specified::Value(CanvasPaint::Solid(_)) => Some(Self::Solid),
            Specified::Value(CanvasPaint::LinearGradient(_)) => Some(Self::LinearGradient),
            Specified::Value(CanvasPaint::RadialGradient(_)) => Some(Self::RadialGradient),
            Specified::Value(CanvasPaint::Pattern(_)) => Some(Self::Pattern),
        }
    }

    const fn is_legacy_scalar(self) -> bool {
        match self {
            Self::Transparent | Self::Solid => true,
            Self::Clear | Self::LinearGradient | Self::RadialGradient | Self::Pattern => false,
        }
    }
}

impl FamilyThemeRuleFacet {
    pub(crate) fn fill(paint: &Specified<CanvasPaint>) -> Option<Self> {
        FamilyThemePaintKind::from_specified(paint).map(Self::Fill)
    }

    pub(crate) fn stroke(paint: &Specified<CanvasPaint>) -> Option<Self> {
        FamilyThemePaintKind::from_specified(paint).map(Self::Stroke)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemeRuleFacet {
    Fill(FamilyThemePaintKind),
    Stroke(FamilyThemePaintKind),
    StrokeWidth,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
    Opacity,
    FillOpacity,
    StrokeOpacity,
    Radius,
    Padding,
    Typography(ThemeTypographyProperty),
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemeSelectorShape {
    Static {
        variant: Option<ThemeVariant>,
    },
    Ordinal {
        variant: Option<ThemeVariant>,
        selector: OrdinalSelector,
    },
}

impl FamilyThemeSelectorShape {
    fn from_rule(rule: &ThemeRule) -> Self {
        match rule.ordinal() {
            None => Self::Static {
                variant: rule.variant(),
            },
            Some(selector @ OrdinalSelector::Exact(_)) => Self::Ordinal {
                variant: rule.variant(),
                selector,
            },
            Some(selector @ OrdinalSelector::Cycle { .. }) => Self::Ordinal {
                variant: rule.variant(),
                selector,
            },
        }
    }

    const fn static_variant(self) -> Option<Option<ThemeVariant>> {
        match self {
            Self::Static { variant } => Some(variant),
            Self::Ordinal { .. } => None,
        }
    }

    pub(crate) const fn ordinal_domain_intersects_occurrence_count(
        self,
        occurrence_count: usize,
    ) -> bool {
        match self {
            Self::Static { .. } => occurrence_count != 0,
            Self::Ordinal {
                selector: OrdinalSelector::Exact(index),
                ..
            } => index <= occurrence_count,
            Self::Ordinal {
                selector: OrdinalSelector::Cycle { offset, .. },
                ..
            } => offset < occurrence_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemeMechanism {
    BaseTypography(ThemeTypographyProperty),
    RuleFacet {
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    },
    OrdinalPalette {
        target: ThemeTarget,
    },
    EffectBinding {
        binding_index: usize,
        target: ThemeTarget,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FamilyThemeRoute {
    mechanism: FamilyThemeMechanism,
    disposition: FamilyThemeDisposition,
}

impl FamilyThemeRoute {
    const fn new(mechanism: FamilyThemeMechanism, disposition: FamilyThemeDisposition) -> Self {
        Self {
            mechanism,
            disposition,
        }
    }

    pub(crate) const fn mechanism(self) -> FamilyThemeMechanism {
        self.mechanism
    }

    pub(crate) const fn disposition(self) -> FamilyThemeDisposition {
        self.disposition
    }
}

pub(super) fn compile_base_typography_routes(
    family: DiagramFamilyId,
    style: &TextStyle,
) -> Vec<FamilyThemeRoute> {
    let mut routes = Vec::new();
    if style.is_specified(ThemeTypographyProperty::FontStack) {
        let disposition = classify_base_typography(family, ThemeTypographyProperty::FontStack);
        let disposition = if disposition == FamilyThemeDisposition::LegacyCompatibility
            && style.font_stack().as_css().len() > MAX_LEGACY_ASSIGNMENT_STRING_BYTES
        {
            FamilyThemeDisposition::Unsupported
        } else {
            disposition
        };
        routes.push(FamilyThemeRoute::new(
            FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack),
            disposition,
        ));
    }
    if style.is_specified(ThemeTypographyProperty::FontSize) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontSize);
    }
    if style.is_specified(ThemeTypographyProperty::FontWeight) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontWeight);
    }
    if style.is_specified(ThemeTypographyProperty::FontStyle) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontStyle);
    }
    if style.is_specified(ThemeTypographyProperty::LineHeight) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::LineHeight);
    }
    if style.is_specified(ThemeTypographyProperty::LetterSpacing) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::LetterSpacing);
    }
    if style.is_specified(ThemeTypographyProperty::WordSpacing) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::WordSpacing);
    }
    if style.is_specified(ThemeTypographyProperty::Transform) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::Transform);
    }
    if style.is_specified(ThemeTypographyProperty::Decoration) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::Decoration);
    }
    if style.is_specified(ThemeTypographyProperty::TextAlign) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::TextAlign);
    }
    if style.is_specified(ThemeTypographyProperty::WhiteSpace) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::WhiteSpace);
    }
    if style.is_specified(ThemeTypographyProperty::Wrap) {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::Wrap);
    }
    routes
}

fn push_base_typography(
    routes: &mut Vec<FamilyThemeRoute>,
    family: DiagramFamilyId,
    property: ThemeTypographyProperty,
) {
    routes.push(FamilyThemeRoute::new(
        FamilyThemeMechanism::BaseTypography(property),
        classify_base_typography(family, property),
    ));
}

pub(super) fn compile_rule_routes(
    family: DiagramFamilyId,
    rule_index: usize,
    rule: &ThemeRule,
) -> Vec<FamilyThemeRoute> {
    let target = rule.target();
    let selector = FamilyThemeSelectorShape::from_rule(rule);
    let style = rule.style();
    let mut routes = Vec::new();
    if let Some(kind) = FamilyThemePaintKind::from_specified(&style.paint.fill) {
        push_rule_facet(
            &mut routes,
            family,
            rule_index,
            target,
            selector,
            FamilyThemeRuleFacet::Fill(kind),
        );
    }
    if let Some(kind) = FamilyThemePaintKind::from_specified(&style.stroke.paint) {
        push_rule_facet(
            &mut routes,
            family,
            rule_index,
            target,
            selector,
            FamilyThemeRuleFacet::Stroke(kind),
        );
    }
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.stroke.width,
        FamilyThemeRuleFacet::StrokeWidth,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.stroke.dasharray,
        FamilyThemeRuleFacet::StrokeDasharray,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.stroke.linecap,
        FamilyThemeRuleFacet::StrokeLinecap,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.stroke.linejoin,
        FamilyThemeRuleFacet::StrokeLinejoin,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.paint.opacity,
        FamilyThemeRuleFacet::Opacity,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.paint.fill_opacity,
        FamilyThemeRuleFacet::FillOpacity,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.stroke.stroke_opacity,
        FamilyThemeRuleFacet::StrokeOpacity,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.geometry.radius,
        FamilyThemeRuleFacet::Radius,
    );
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.spacing.padding,
        FamilyThemeRuleFacet::Padding,
    );

    macro_rules! push_typography {
        ($field:ident, $property:ident) => {
            push_specified_rule_facet(
                &mut routes,
                family,
                rule_index,
                target,
                selector,
                &style.typography.$field,
                FamilyThemeRuleFacet::Typography(ThemeTypographyProperty::$property),
            );
        };
    }
    push_typography!(font_stack, FontStack);
    push_typography!(font_size_px, FontSize);
    push_typography!(font_weight, FontWeight);
    push_typography!(font_style, FontStyle);
    push_typography!(line_height, LineHeight);
    push_typography!(letter_spacing_px, LetterSpacing);
    push_typography!(word_spacing_px, WordSpacing);
    push_typography!(transform, Transform);
    push_typography!(decoration, Decoration);
    push_typography!(text_align, TextAlign);
    push_typography!(white_space, WhiteSpace);
    push_typography!(wrap, Wrap);
    push_specified_rule_facet(
        &mut routes,
        family,
        rule_index,
        target,
        selector,
        &style.effects.effect,
        FamilyThemeRuleFacet::Effect,
    );
    routes
}

#[allow(clippy::too_many_arguments)]
fn push_specified_rule_facet<T>(
    routes: &mut Vec<FamilyThemeRoute>,
    family: DiagramFamilyId,
    rule_index: usize,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    specified: &Specified<T>,
    facet: FamilyThemeRuleFacet,
) {
    if !matches!(specified, Specified::Unspecified) {
        push_rule_facet(routes, family, rule_index, target, selector, facet);
    }
}

fn push_rule_facet(
    routes: &mut Vec<FamilyThemeRoute>,
    family: DiagramFamilyId,
    rule_index: usize,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    routes.push(FamilyThemeRoute::new(
        FamilyThemeMechanism::RuleFacet {
            rule_index,
            target,
            selector,
            facet,
        },
        classify_rule_facet(family, target, selector, facet),
    ));
}

pub(super) fn compile_ordinal_palette_route(
    family: DiagramFamilyId,
    target: ThemeTarget,
) -> FamilyThemeRoute {
    FamilyThemeRoute::new(
        FamilyThemeMechanism::OrdinalPalette { target },
        classify_ordinal_palette(family, target),
    )
}

fn classify_ordinal_palette(
    family: DiagramFamilyId,
    target: ThemeTarget,
) -> FamilyThemeDisposition {
    if family == DiagramFamilyId::STATE {
        return if matches!(
            target,
            ThemeTarget::State
                | ThemeTarget::StateLabel
                | ThemeTarget::Transition
                | ThemeTarget::TransitionLabel
                | ThemeTarget::Composite
                | ThemeTarget::CompositeLabel
                | ThemeTarget::SpecialState
                | ThemeTarget::Note
                | ThemeTarget::NoteLabel
        ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if (matches!(
        family,
        DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::MINDMAP
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::SANKEY
    ) && target == ThemeTarget::Node)
        || (family == DiagramFamilyId::PIE && target == ThemeTarget::PieSlice)
        || (family == DiagramFamilyId::KANBAN && target == ThemeTarget::Task)
        || (family == DiagramFamilyId::JOURNEY && target == ThemeTarget::JourneyTask)
        || (family == DiagramFamilyId::TIMELINE && target == ThemeTarget::TimelineEvent)
        || (family == DiagramFamilyId::XY_CHART && target == ThemeTarget::ChartSeries)
        || (family == DiagramFamilyId::RADAR && target == ThemeTarget::ChartSeries)
    {
        FamilyThemeDisposition::TypedAdapter
    } else {
        FamilyThemeDisposition::Unsupported
    }
}

/// Projects the private route domain into the coarse facts used by public support discovery.
///
/// This deliberately returns only aggregate disposition presence. Selectors, paint classes, route
/// identities, and writer evidence remain private implementation details.
#[cfg(test)]
pub(super) fn summarize_theme_support(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeSupportFacetV1,
) -> FamilyThemeSupportSummary {
    let mut summary = FamilyThemeSupportSummary::default();
    let rule_facet = match facet {
        ThemeSupportFacetV1::OrdinalPalette => {
            summary.record(classify_ordinal_palette(family, target));
            return summary;
        }
        ThemeSupportFacetV1::Rule(rule_facet) => rule_facet,
        _ => return summary,
    };

    let effect_binding =
        (rule_facet == ThemeRuleFacetV1::Effect).then(|| classify_effect_binding(family, target));
    for_each_matrix_selector(|selector| {
        for_each_public_rule_facet(rule_facet, |matrix_facet| {
            let mut disposition = classify_rule_facet(family, target, selector, matrix_facet);
            if effect_binding.is_some_and(|binding| {
                disposition != FamilyThemeDisposition::TypedAdapter
                    || binding != FamilyThemeDisposition::TypedAdapter
            }) {
                disposition = FamilyThemeDisposition::Unsupported;
            }
            summary.record(disposition);
        });
    });
    if family == DiagramFamilyId::STATE {
        // Paint already accounts for unsupported value kinds. Other facets do not encode Clear
        // or exact terminal-surface admission, so State's typed routes still have a partial domain.
        summary.record(FamilyThemeDisposition::Unsupported);
    }
    summary
}

/// Projects the family-wide base typography classifier into public V2 discovery facts.
///
/// The property match is only a contract-to-renderer vocabulary translation. Route ownership
/// remains exclusively defined by `classify_base_typography`.
#[cfg(test)]
pub(super) fn summarize_base_typography_support(
    family: DiagramFamilyId,
    property: ThemeSupportBaseTypographyPropertyV1,
) -> FamilyThemeSupportSummary {
    let property = match property {
        ThemeSupportBaseTypographyPropertyV1::FontStack => ThemeTypographyProperty::FontStack,
        ThemeSupportBaseTypographyPropertyV1::FontSize => ThemeTypographyProperty::FontSize,
        ThemeSupportBaseTypographyPropertyV1::FontWeight => ThemeTypographyProperty::FontWeight,
        ThemeSupportBaseTypographyPropertyV1::FontStyle => ThemeTypographyProperty::FontStyle,
        ThemeSupportBaseTypographyPropertyV1::LineHeight => ThemeTypographyProperty::LineHeight,
        ThemeSupportBaseTypographyPropertyV1::LetterSpacing => {
            ThemeTypographyProperty::LetterSpacing
        }
        ThemeSupportBaseTypographyPropertyV1::WordSpacing => ThemeTypographyProperty::WordSpacing,
        ThemeSupportBaseTypographyPropertyV1::Transform => ThemeTypographyProperty::Transform,
        ThemeSupportBaseTypographyPropertyV1::Decoration => ThemeTypographyProperty::Decoration,
        ThemeSupportBaseTypographyPropertyV1::TextAlign => ThemeTypographyProperty::TextAlign,
        ThemeSupportBaseTypographyPropertyV1::WhiteSpace => ThemeTypographyProperty::WhiteSpace,
        ThemeSupportBaseTypographyPropertyV1::Wrap => ThemeTypographyProperty::Wrap,
        _ => return FamilyThemeSupportSummary::default(),
    };
    let mut summary = FamilyThemeSupportSummary::default();
    summary.record(classify_base_typography(family, property));
    summary
}

#[cfg(test)]
fn for_each_public_rule_facet(
    facet: ThemeRuleFacetV1,
    mut visit: impl FnMut(FamilyThemeRuleFacet),
) {
    match facet {
        ThemeRuleFacetV1::Fill | ThemeRuleFacetV1::StrokePaint => {
            for kind in FamilyThemePaintKind::ALL {
                visit(if facet == ThemeRuleFacetV1::Fill {
                    FamilyThemeRuleFacet::Fill(kind)
                } else {
                    FamilyThemeRuleFacet::Stroke(kind)
                });
            }
        }
        ThemeRuleFacetV1::Opacity => visit(FamilyThemeRuleFacet::Opacity),
        ThemeRuleFacetV1::FillOpacity => visit(FamilyThemeRuleFacet::FillOpacity),
        ThemeRuleFacetV1::StrokeWidth => visit(FamilyThemeRuleFacet::StrokeWidth),
        ThemeRuleFacetV1::StrokeDasharray => visit(FamilyThemeRuleFacet::StrokeDasharray),
        ThemeRuleFacetV1::StrokeLineCap => visit(FamilyThemeRuleFacet::StrokeLinecap),
        ThemeRuleFacetV1::StrokeLineJoin => visit(FamilyThemeRuleFacet::StrokeLinejoin),
        ThemeRuleFacetV1::StrokeOpacity => visit(FamilyThemeRuleFacet::StrokeOpacity),
        ThemeRuleFacetV1::Radius => visit(FamilyThemeRuleFacet::Radius),
        ThemeRuleFacetV1::Padding => visit(FamilyThemeRuleFacet::Padding),
        ThemeRuleFacetV1::FontStack => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::FontStack,
        )),
        ThemeRuleFacetV1::FontSize => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::FontSize,
        )),
        ThemeRuleFacetV1::FontWeight => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::FontWeight,
        )),
        ThemeRuleFacetV1::FontStyle => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::FontStyle,
        )),
        ThemeRuleFacetV1::LineHeight => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::LineHeight,
        )),
        ThemeRuleFacetV1::LetterSpacing => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::LetterSpacing,
        )),
        ThemeRuleFacetV1::WordSpacing => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::WordSpacing,
        )),
        ThemeRuleFacetV1::TextTransform => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::Transform,
        )),
        ThemeRuleFacetV1::TextDecoration => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::Decoration,
        )),
        ThemeRuleFacetV1::TextAlign => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::TextAlign,
        )),
        ThemeRuleFacetV1::WhiteSpace => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::WhiteSpace,
        )),
        ThemeRuleFacetV1::Wrap => visit(FamilyThemeRuleFacet::Typography(
            ThemeTypographyProperty::Wrap,
        )),
        ThemeRuleFacetV1::Effect => visit(FamilyThemeRuleFacet::Effect),
    }
}

pub(super) fn compile_effect_binding_route(
    family: DiagramFamilyId,
    binding_index: usize,
    target: ThemeTarget,
) -> FamilyThemeRoute {
    FamilyThemeRoute::new(
        FamilyThemeMechanism::EffectBinding {
            binding_index,
            target,
        },
        classify_effect_binding(family, target),
    )
}

fn classify_effect_binding(family: DiagramFamilyId, target: ThemeTarget) -> FamilyThemeDisposition {
    if (family == DiagramFamilyId::STATE && target == ThemeTarget::State)
        || (family == DiagramFamilyId::SEQUENCE
            && matches!(
                target,
                ThemeTarget::Actor
                    | ThemeTarget::ActorLabel
                    | ThemeTarget::Message
                    | ThemeTarget::Lifeline
                    | ThemeTarget::Note
                    | ThemeTarget::Activation
                    | ThemeTarget::NoteLabel
                    | ThemeTarget::LoopLabel
                    | ThemeTarget::Loop
                    | ThemeTarget::LoopLabelBackground
            ))
        || (family == DiagramFamilyId::XY_CHART
            && matches!(
                target,
                ThemeTarget::ChartSeries
                    | ThemeTarget::Title
                    | ThemeTarget::AxisTitle
                    | ThemeTarget::AxisLabel
                    | ThemeTarget::Legend
            ))
        || (matches!(
            family,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ) && matches!(target, ThemeTarget::Node | ThemeTarget::Edge))
        || (family == DiagramFamilyId::FLOWCHART
            && matches!(target, ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel))
    {
        FamilyThemeDisposition::TypedAdapter
    } else {
        FamilyThemeDisposition::Unsupported
    }
}

fn classify_base_typography(
    family: DiagramFamilyId,
    property: ThemeTypographyProperty,
) -> FamilyThemeDisposition {
    if family == DiagramFamilyId::STATE {
        return match property {
            ThemeTypographyProperty::FontStack
            | ThemeTypographyProperty::FontSize
            | ThemeTypographyProperty::FontWeight
            | ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LetterSpacing
            | ThemeTypographyProperty::WordSpacing
            | ThemeTypographyProperty::Transform => FamilyThemeDisposition::TypedAdapter,
            ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::Decoration
            | ThemeTypographyProperty::TextAlign
            | ThemeTypographyProperty::WhiteSpace
            | ThemeTypographyProperty::Wrap => FamilyThemeDisposition::Unsupported,
        };
    }
    match property {
        ThemeTypographyProperty::FontStack => classify_font_stack(family),
        ThemeTypographyProperty::FontSize => classify_font_size(family),
        ThemeTypographyProperty::FontWeight
        | ThemeTypographyProperty::FontStyle
        | ThemeTypographyProperty::LineHeight
        | ThemeTypographyProperty::LetterSpacing
        | ThemeTypographyProperty::WordSpacing
        | ThemeTypographyProperty::Transform
        | ThemeTypographyProperty::Decoration
        | ThemeTypographyProperty::TextAlign
        | ThemeTypographyProperty::WhiteSpace
        | ThemeTypographyProperty::Wrap => FamilyThemeDisposition::Unsupported,
    }
}

fn classify_font_stack(family: DiagramFamilyId) -> FamilyThemeDisposition {
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::SEQUENCE
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::GANTT
            | DiagramFamilyId::MINDMAP
            | DiagramFamilyId::TREE_VIEW
            | DiagramFamilyId::PACKET
            | DiagramFamilyId::BLOCK
            | DiagramFamilyId::SANKEY
            | DiagramFamilyId::RAILROAD
            | DiagramFamilyId::INFO
            | DiagramFamilyId::ERROR
            | DiagramFamilyId::CYNEFIN
            | DiagramFamilyId::WARDLEY
            | DiagramFamilyId::ISHIKAWA
            | DiagramFamilyId::EVENT_MODELING
            | DiagramFamilyId::VENN
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::RADAR
            | DiagramFamilyId::ER
            | DiagramFamilyId::PIE
            | DiagramFamilyId::REQUIREMENT
            | DiagramFamilyId::KANBAN
            | DiagramFamilyId::QUADRANT_CHART
            | DiagramFamilyId::TIMELINE
            | DiagramFamilyId::JOURNEY
            | DiagramFamilyId::C4
            | DiagramFamilyId::ARCHITECTURE
            | DiagramFamilyId::XY_CHART
            | DiagramFamilyId::TREEMAP
    ) {
        return FamilyThemeDisposition::TypedAdapter;
    }
    FamilyThemeDisposition::Unsupported
}

fn classify_font_size(family: DiagramFamilyId) -> FamilyThemeDisposition {
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::SEQUENCE
            | DiagramFamilyId::BLOCK
            | DiagramFamilyId::RAILROAD
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::ISHIKAWA
            | DiagramFamilyId::EVENT_MODELING
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::RADAR
            | DiagramFamilyId::ER
            | DiagramFamilyId::TIMELINE
            | DiagramFamilyId::REQUIREMENT
            | DiagramFamilyId::KANBAN
            | DiagramFamilyId::JOURNEY
            | DiagramFamilyId::C4
            | DiagramFamilyId::ARCHITECTURE
    ) {
        return FamilyThemeDisposition::TypedAdapter;
    }
    FamilyThemeDisposition::Unsupported
}

/// Current terminal ownership for scalar paint routes that have no Mermaid writer consumer.
///
/// This is deliberately independent from KTD23's historical before/after witness. Editing the
/// retirement manifest cannot change runtime classification, and editing this classification
/// cannot authorize its own historical projection deletion.
fn legacy_paint_route_without_writer_consumer(
    family: DiagramFamilyId,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> bool {
    let Some(variant) = (match selector {
        FamilyThemeSelectorShape::Static { variant } => Some(variant),
        FamilyThemeSelectorShape::Ordinal { .. } => None,
    }) else {
        return false;
    };
    let fill = matches!(facet, FamilyThemeRuleFacet::Fill(_));
    let stroke = matches!(facet, FamilyThemeRuleFacet::Stroke(_));
    let unqualified_or_default = matches!(variant, None | Some(ThemeVariant::Default));

    match family {
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE => {
            target == ThemeTarget::Marker && unqualified_or_default && (fill || stroke)
        }
        DiagramFamilyId::CLASS => {
            (target == ThemeTarget::Marker && unqualified_or_default && (fill || stroke))
                || (matches!(target, ThemeTarget::ClusterLabel) && unqualified_or_default && fill)
                || (target == ThemeTarget::Table
                    && fill
                    && matches!(variant, None | Some(ThemeVariant::Odd | ThemeVariant::Even)))
        }
        DiagramFamilyId::MINDMAP => {
            (matches!(
                target,
                ThemeTarget::NodeLabel | ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::Edge
            ) && unqualified_or_default
                && fill)
                || (target == ThemeTarget::Edge && variant == Some(ThemeVariant::Default) && stroke)
                || (target == ThemeTarget::Marker && unqualified_or_default && (fill || stroke))
                || (target == ThemeTarget::EdgeLabelBackground && unqualified_or_default && fill)
                || (target == ThemeTarget::Cluster && unqualified_or_default && (fill || stroke))
                || (target == ThemeTarget::ClusterLabel && unqualified_or_default && fill)
        }
        DiagramFamilyId::TREE_VIEW => {
            (target == ThemeTarget::Node && unqualified_or_default && (fill || stroke))
                || (matches!(
                    target,
                    ThemeTarget::Title | ThemeTarget::EdgeLabelBackground
                ) && unqualified_or_default
                    && fill)
                || (target == ThemeTarget::Cluster && unqualified_or_default && (fill || stroke))
                || (target == ThemeTarget::ClusterLabel && unqualified_or_default && fill)
        }
        DiagramFamilyId::GIT_GRAPH => {
            (target == ThemeTarget::Title && unqualified_or_default && fill)
                || (target == ThemeTarget::Marker && unqualified_or_default && (fill || stroke))
                || (target == ThemeTarget::Cluster && unqualified_or_default && (fill || stroke))
                || (target == ThemeTarget::ClusterLabel && unqualified_or_default && fill)
        }
        DiagramFamilyId::WARDLEY | DiagramFamilyId::SEQUENCE => {
            matches!(target, ThemeTarget::Text | ThemeTarget::Title)
                && unqualified_or_default
                && fill
        }
        DiagramFamilyId::ER | DiagramFamilyId::JOURNEY | DiagramFamilyId::KANBAN => {
            target == ThemeTarget::Title && unqualified_or_default && fill
        }
        DiagramFamilyId::BLOCK => {
            matches!(target, ThemeTarget::Title | ThemeTarget::ClusterLabel)
                && unqualified_or_default
                && fill
        }
        DiagramFamilyId::ARCHITECTURE
        | DiagramFamilyId::C4
        | DiagramFamilyId::CYNEFIN
        | DiagramFamilyId::SANKEY => target == ThemeTarget::Title && unqualified_or_default && fill,
        _ => false,
    }
}

/// State's possible terminal consumers, before source ownership and value-dependent admission.
fn classify_state_rule_facet(
    target: ThemeTarget,
    facet: FamilyThemeRuleFacet,
) -> FamilyThemeDisposition {
    let text_surface = matches!(
        target,
        ThemeTarget::Text
            | ThemeTarget::Title
            | ThemeTarget::StateLabel
            | ThemeTarget::TransitionLabel
            | ThemeTarget::CompositeLabel
            | ThemeTarget::NoteLabel
    );
    let shape_surface = matches!(
        target,
        ThemeTarget::State
            | ThemeTarget::Transition
            | ThemeTarget::TransitionMarker
            | ThemeTarget::TransitionLabelBackground
            | ThemeTarget::Composite
            | ThemeTarget::CompositeHeader
            | ThemeTarget::SpecialState
            | ThemeTarget::SpecialStateInner
            | ThemeTarget::Note
    );
    let supported = match facet {
        FamilyThemeRuleFacet::Fill(
            FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent,
        ) => text_surface || shape_surface,
        FamilyThemeRuleFacet::Stroke(
            FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent,
        ) => shape_surface,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => false,
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity => shape_surface,
        FamilyThemeRuleFacet::Radius | FamilyThemeRuleFacet::Padding => matches!(
            target,
            ThemeTarget::State | ThemeTarget::Composite | ThemeTarget::Note
        ),
        FamilyThemeRuleFacet::Typography(property) => {
            text_surface
                && classify_base_typography(DiagramFamilyId::STATE, property)
                    == FamilyThemeDisposition::TypedAdapter
        }
        // Clear suppresses a target binding on all node surfaces. A non-clear graph is admitted
        // only for a classic State rectangle; the terminal adapter retains that distinction.
        FamilyThemeRuleFacet::Effect => matches!(
            target,
            ThemeTarget::State
                | ThemeTarget::Composite
                | ThemeTarget::SpecialState
                | ThemeTarget::Note
        ),
    };
    if supported {
        FamilyThemeDisposition::TypedAdapter
    } else {
        FamilyThemeDisposition::Unsupported
    }
}

pub(super) fn classify_rule_facet(
    family: DiagramFamilyId,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> FamilyThemeDisposition {
    if family == DiagramFamilyId::XY_CHART
        && matches!(
            target,
            ThemeTarget::Title
                | ThemeTarget::AxisTitle
                | ThemeTarget::AxisLabel
                | ThemeTarget::Legend
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Effect
                | FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Solid
                        | FamilyThemePaintKind::Transparent
                        | FamilyThemePaintKind::Clear
                )
                | FamilyThemeRuleFacet::Typography(
                    ThemeTypographyProperty::FontSize | ThemeTypographyProperty::FontWeight
                )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if target == ThemeTarget::AxisTick {
        return if family == DiagramFamilyId::XY_CHART
            && matches!(
                selector,
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default)
                }
            )
            && matches!(
                facet,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Solid
                        | FamilyThemePaintKind::Transparent
                        | FamilyThemePaintKind::Clear
                ) | FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Solid
                        | FamilyThemePaintKind::Transparent
                        | FamilyThemePaintKind::Clear
                ) | FamilyThemeRuleFacet::Opacity
            ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if matches!(target, ThemeTarget::AxisTitle | ThemeTarget::AxisLabel) {
        return FamilyThemeDisposition::Unsupported;
    }
    if family == DiagramFamilyId::XY_CHART
        && target == ThemeTarget::ChartSeries
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None
                    | Some(ThemeVariant::Default | ThemeVariant::Bar | ThemeVariant::Line)
            } | FamilyThemeSelectorShape::Ordinal {
                variant: None
                    | Some(ThemeVariant::Default | ThemeVariant::Bar | ThemeVariant::Line),
                ..
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Clear
                    | FamilyThemePaintKind::Solid
                    | FamilyThemePaintKind::Transparent
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Clear
                    | FamilyThemePaintKind::Solid
                    | FamilyThemePaintKind::Transparent
            ) | FamilyThemeRuleFacet::StrokeWidth
                | FamilyThemeRuleFacet::Opacity
                | FamilyThemeRuleFacet::FillOpacity
                | FamilyThemeRuleFacet::StrokeOpacity
                | FamilyThemeRuleFacet::Effect
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }

    // Plot-kind variants have no corresponding terminal in other family/target domains.
    if matches!(
        selector,
        FamilyThemeSelectorShape::Static {
            variant: Some(ThemeVariant::Bar | ThemeVariant::Line)
        } | FamilyThemeSelectorShape::Ordinal {
            variant: Some(ThemeVariant::Bar | ThemeVariant::Line),
            ..
        }
    ) {
        return FamilyThemeDisposition::Unsupported;
    }

    if family == DiagramFamilyId::STATE {
        return classify_state_rule_facet(target, facet);
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && matches!(target, ThemeTarget::Node | ThemeTarget::Edge)
        && facet == FamilyThemeRuleFacet::Effect
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            } | FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            }
        )
        && (target == ThemeTarget::Node
            || matches!(selector, FamilyThemeSelectorShape::Static { .. }))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::FLOWCHART
        && matches!(target, ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel)
        && facet == FamilyThemeRuleFacet::Effect
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            } | FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            }
        )
        && (target == ThemeTarget::NodeLabel
            || matches!(selector, FamilyThemeSelectorShape::Static { .. }))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::PACKET
        && matches!(
            target,
            ThemeTarget::Text
                | ThemeTarget::PacketByteLabel
                | ThemeTarget::PacketFieldLabel
                | ThemeTarget::Title
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SANKEY
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::INFO
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::BLOCK | DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::EdgeLabelBackground
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::BLOCK | DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && matches!(target, ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel)
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && ((matches!(
            family,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ) && matches!(
            facet,
            FamilyThemeRuleFacet::Typography(
                ThemeTypographyProperty::FontStack
                    | ThemeTypographyProperty::FontSize
                    | ThemeTypographyProperty::FontWeight
            )
        )) || (matches!(
            family,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ) && target == ThemeTarget::EdgeLabel
            && facet == FamilyThemeRuleFacet::Padding)
            || (target == ThemeTarget::NodeLabel
                && matches!(
                    facet,
                    FamilyThemeRuleFacet::Fill(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                    )
                )))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            target,
            ThemeTarget::Actor
                | ThemeTarget::LoopLabelBackground
                | ThemeTarget::Note
                | ThemeTarget::Activation
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && ((target == ThemeTarget::Loop
            && matches!(
                facet,
                FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent
                ) | FamilyThemeRuleFacet::StrokeWidth
                    | FamilyThemeRuleFacet::Effect
            ))
            || (target == ThemeTarget::LoopLabelBackground
                && matches!(
                    facet,
                    FamilyThemeRuleFacet::StrokeWidth | FamilyThemeRuleFacet::Effect
                )))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            target,
            ThemeTarget::Actor | ThemeTarget::Note | ThemeTarget::Activation
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::StrokeWidth
                | FamilyThemeRuleFacet::Radius
                | FamilyThemeRuleFacet::Effect
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && target == ThemeTarget::SequenceNumberLabel
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && target == ThemeTarget::Message
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::StrokeWidth
                | FamilyThemeRuleFacet::Effect
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && target == ThemeTarget::Lifeline
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Effect
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && target == ThemeTarget::Lifeline
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::StrokeWidth
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            target,
            ThemeTarget::ActorLabel | ThemeTarget::NoteLabel | ThemeTarget::LoopLabel
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && facet == FamilyThemeRuleFacet::Effect
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            target,
            ThemeTarget::ActorLabel
                | ThemeTarget::MessageLabel
                | ThemeTarget::NoteLabel
                | ThemeTarget::LoopLabel
        )
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Typography(
                ThemeTypographyProperty::FontStack
                    | ThemeTypographyProperty::FontSize
                    | ThemeTypographyProperty::FontWeight
                    | ThemeTypographyProperty::FontStyle
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(
            target,
            ThemeTarget::ActorLabel
                | ThemeTarget::MessageLabel
                | ThemeTarget::NoteLabel
                | ThemeTarget::LoopLabel
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && matches!(
        target,
        ThemeTarget::Cluster | ThemeTarget::ClusterLabel | ThemeTarget::Title | ThemeTarget::Text
    ) && matches!(
        selector,
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default)
        }
    ) && matches!(
        facet,
        FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid)
            | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
    ) && (target == ThemeTarget::Cluster || matches!(facet, FamilyThemeRuleFacet::Fill(_)))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Cluster
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && facet == FamilyThemeRuleFacet::StrokeWidth
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ARCHITECTURE
        && target == ThemeTarget::Cluster
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Clear
                    | FamilyThemePaintKind::Transparent
                    | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Clear
                    | FamilyThemePaintKind::Transparent
                    | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ARCHITECTURE
        && target == ThemeTarget::Edge
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ARCHITECTURE
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ARCHITECTURE
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            (target, facet),
            (
                ThemeTarget::Node,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                ) | FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) | (
                ThemeTarget::Marker,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::CLASS
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            (target, facet),
            (
                ThemeTarget::Edge | ThemeTarget::Node | ThemeTarget::Cluster,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                ) | FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) | (
                ThemeTarget::NodeLabel
                    | ThemeTarget::Title
                    | ThemeTarget::Text
                    | ThemeTarget::EdgeLabelBackground,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::MINDMAP
        && target == ThemeTarget::Node
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if ((family == DiagramFamilyId::MINDMAP
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None }))
        || (family == DiagramFamilyId::GIT_GRAPH
            && matches!(
                selector,
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default)
                }
            )))
        && (target == ThemeTarget::Edge
            || (family == DiagramFamilyId::GIT_GRAPH && target == ThemeTarget::Node))
        && matches!(
            facet,
            FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::GIT_GRAPH
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            target,
            ThemeTarget::Node
                | ThemeTarget::Edge
                | ThemeTarget::EdgeLabelBackground
                | ThemeTarget::NodeLabel
                | ThemeTarget::EdgeLabel
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TREE_VIEW
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            (target, facet),
            (
                ThemeTarget::NodeLabel | ThemeTarget::Text,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) | (
                ThemeTarget::Edge,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                ) | FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) | (
                ThemeTarget::Marker,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                ) | FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if legacy_paint_route_without_writer_consumer(family, target, selector, facet) {
        return FamilyThemeDisposition::Unsupported;
    }
    if family == DiagramFamilyId::CLASS
        && target == ThemeTarget::Edge
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && facet == FamilyThemeRuleFacet::StrokeWidth
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(family, DiagramFamilyId::GANTT | DiagramFamilyId::KANBAN)
        && target == ThemeTarget::Task
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::Radius
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::KANBAN
        && target == ThemeTarget::Task
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::KANBAN
        && target == ThemeTarget::Task
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::GANTT
        && target == ThemeTarget::Task
        && (matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None
                    | Some(ThemeVariant::Default)
                    | Some(ThemeVariant::Active)
                    | Some(ThemeVariant::Success)
                    | Some(ThemeVariant::Error)
            }
        ) || (matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: Some(ThemeVariant::Warning)
            }
        ) && matches!(facet, FamilyThemeRuleFacet::Stroke(_))))
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::GANTT
        && target == ThemeTarget::Title
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::GANTT
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::KANBAN
        && target == ThemeTarget::TaskLabel
        && matches!(
            selector,
            FamilyThemeSelectorShape::Ordinal { variant: None, .. }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TIMELINE
        && target == ThemeTarget::TimelineEvent
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Radius | FamilyThemeRuleFacet::Opacity
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TIMELINE
        && target == ThemeTarget::TimelineEvent
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TIMELINE
        && matches!(target, ThemeTarget::TimelineEvent | ThemeTarget::Text)
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TREE_VIEW
        && target == ThemeTarget::Edge
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::StrokeWidth
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::JOURNEY
            | DiagramFamilyId::KANBAN
            | DiagramFamilyId::GIT_GRAPH
            | DiagramFamilyId::QUADRANT_CHART
            | DiagramFamilyId::XY_CHART
    ) && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::JOURNEY
        && target == ThemeTarget::JourneyTask
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::Radius
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::JOURNEY
        && target == ThemeTarget::JourneyTask
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::QUADRANT_CHART | DiagramFamilyId::XY_CHART
    ) && target == ThemeTarget::Axis
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Solid | FamilyThemePaintKind::Transparent
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::QUADRANT_CHART | DiagramFamilyId::XY_CHART
    ) && target == ThemeTarget::Title
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::QUADRANT_CHART
        && target == ThemeTarget::ChartSeries
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::Radius
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::QUADRANT_CHART
        && target == ThemeTarget::ChartSeries
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::C4
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::C4
        && target == ThemeTarget::Cluster
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && facet == FamilyThemeRuleFacet::Radius
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::C4
        && target == ThemeTarget::Cluster
        && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::TREEMAP && target == ThemeTarget::Title {
        return if matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
            && matches!(
                facet,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if family == DiagramFamilyId::TREEMAP
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::VENN && target == ThemeTarget::Title {
        return if matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
            && matches!(
                facet,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if family == DiagramFamilyId::VENN
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ZENUML && target == ThemeTarget::Title {
        return if matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
            && matches!(
                facet,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if family == DiagramFamilyId::ZENUML && target == ThemeTarget::Text {
        return FamilyThemeDisposition::Unsupported;
    }
    if family == DiagramFamilyId::CYNEFIN
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::EVENT_MODELING | DiagramFamilyId::ISHIKAWA
    ) && matches!(target, ThemeTarget::Text | ThemeTarget::Title)
    {
        return if target == ThemeTarget::Text
            && matches!(selector, FamilyThemeSelectorShape::Static { variant: None })
            && matches!(
                facet,
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            ) {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        };
    }
    if family == DiagramFamilyId::REQUIREMENT
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::REQUIREMENT
        && matches!(target, ThemeTarget::Requirement | ThemeTarget::Relation)
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ER
        && target == ThemeTarget::Entity
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ER
        && target == ThemeTarget::Table
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Odd | ThemeVariant::Even)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ER
        && target == ThemeTarget::Relation
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::ER
        && matches!(
            (target, selector, facet),
            (
                ThemeTarget::Text,
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default)
                },
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                )
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        (family, target),
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice)
    ) && matches!(
        selector,
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default)
        }
    ) && matches!(
        facet,
        FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid)
            | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
    ) {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        (family, target),
        (
            DiagramFamilyId::PIE | DiagramFamilyId::RAILROAD,
            ThemeTarget::Title
        )
    ) && matches!(
        selector,
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default)
        }
    ) && matches!(
        facet,
        FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid)
    ) {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::RAILROAD
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::PIE
        && target == ThemeTarget::Text
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::RADAR
        && target == ThemeTarget::Axis
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::RADAR
        && matches!(target, ThemeTarget::Title | ThemeTarget::Text)
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::BLOCK
        && matches!(
            target,
            ThemeTarget::Text | ThemeTarget::Node | ThemeTarget::Cluster | ThemeTarget::Marker
        )
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && (target != ThemeTarget::Text || matches!(facet, FamilyThemeRuleFacet::Fill(_)))
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Edge
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::StrokeWidth | FamilyThemeRuleFacet::StrokeDasharray
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE | DiagramFamilyId::BLOCK
    ) && target == ThemeTarget::Edge
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    // Node paint is resolved per emitted occurrence, including ordinal fill winners.
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Node
        && matches!(
            selector,
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Node
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            ) | FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
            )
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Node
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && matches!(
            facet,
            FamilyThemeRuleFacet::StrokeWidth
                | FamilyThemeRuleFacet::StrokeDasharray
                | FamilyThemeRuleFacet::Radius
        )
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    let Some(variant) = selector.static_variant() else {
        return FamilyThemeDisposition::Unsupported;
    };
    match facet {
        FamilyThemeRuleFacet::Fill(kind)
            if kind.is_legacy_scalar()
                && legacy_paint_supported(family, target, variant, PaintChannel::Fill) =>
        {
            FamilyThemeDisposition::LegacyCompatibility
        }
        FamilyThemeRuleFacet::Stroke(kind)
            if kind.is_legacy_scalar()
                && legacy_paint_supported(family, target, variant, PaintChannel::Stroke) =>
        {
            FamilyThemeDisposition::LegacyCompatibility
        }
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => {
            FamilyThemeDisposition::Unsupported
        }
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding
        | FamilyThemeRuleFacet::Typography(_)
        | FamilyThemeRuleFacet::Effect => FamilyThemeDisposition::Unsupported,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintChannel {
    Fill,
    Stroke,
}

fn legacy_paint_supported(
    family: DiagramFamilyId,
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    channel: PaintChannel,
) -> bool {
    if family == DiagramFamilyId::KANBAN
        && target == ThemeTarget::Task
        && channel == PaintChannel::Stroke
    {
        return variant.is_none();
    }
    let variants = legacy_paint_variants(family, target, channel);
    !variants.is_empty() && variant.is_none_or(|variant| variants.contains(&variant))
}

fn legacy_paint_variants(
    family: DiagramFamilyId,
    target: ThemeTarget,
    channel: PaintChannel,
) -> &'static [ThemeVariant] {
    use DiagramFamilyId as Family;
    use PaintChannel::{Fill, Stroke};
    use ThemeTarget as Target;
    use ThemeVariant as Variant;

    const DEFAULT: &[Variant] = &[Variant::Default];
    const ODD_EVEN: &[Variant] = &[Variant::Odd, Variant::Even];
    const GANTT_FILL: &[Variant] = &[
        Variant::Default,
        Variant::Active,
        Variant::Success,
        Variant::Error,
    ];
    const GANTT_STROKE: &[Variant] = &[
        Variant::Default,
        Variant::Active,
        Variant::Success,
        Variant::Error,
        Variant::Warning,
    ];

    if family == Family::CLASS {
        return match channel {
            Fill if matches!(
                target,
                Target::Node
                    | Target::NodeLabel
                    | Target::Text
                    | Target::Title
                    | Target::Edge
                    | Target::EdgeLabelBackground
                    | Target::Cluster
            ) =>
            {
                DEFAULT
            }
            Stroke if matches!(target, Target::Node | Target::Edge | Target::Cluster) => DEFAULT,
            Fill | Stroke => &[],
        };
    }

    if family == Family::MINDMAP {
        return match channel {
            Fill | Stroke if matches!(target, Target::Node | Target::Edge) => DEFAULT,
            Fill | Stroke => &[],
        };
    }

    if family == Family::TREE_VIEW {
        return match channel {
            Fill if matches!(
                target,
                Target::NodeLabel | Target::Text | Target::Edge | Target::Marker
            ) =>
            {
                DEFAULT
            }
            Stroke if matches!(target, Target::Edge | Target::Marker) => DEFAULT,
            Fill | Stroke => &[],
        };
    }

    if family == Family::GIT_GRAPH {
        return match channel {
            Fill if matches!(
                target,
                Target::Node
                    | Target::NodeLabel
                    | Target::Text
                    | Target::Edge
                    | Target::EdgeLabel
                    | Target::EdgeLabelBackground
            ) =>
            {
                DEFAULT
            }
            Stroke if matches!(target, Target::Node | Target::Edge) => DEFAULT,
            Fill | Stroke => &[],
        };
    }

    let node_family = matches!(family, Family::FLOWCHART | Family::SWIMLANE | Family::BLOCK);
    if node_family {
        let common = match channel {
            Fill => {
                matches!(
                    target,
                    Target::Node
                        | Target::NodeLabel
                        | Target::Text
                        | Target::Title
                        | Target::Edge
                        | Target::Marker
                        | Target::EdgeLabelBackground
                        | Target::Cluster
                        | Target::ClusterLabel
                )
            }
            Stroke => matches!(
                target,
                Target::Node | Target::Edge | Target::Marker | Target::Cluster
            ),
        };
        if common {
            return DEFAULT;
        }
        return &[];
    }

    match family {
        Family::RAILROAD => match (target, channel) {
            (Target::Text | Target::Title, Fill) => DEFAULT,
            _ => &[],
        },
        Family::SEQUENCE => match channel {
            Fill if matches!(
                target,
                Target::Actor
                    | Target::ActorLabel
                    | Target::Text
                    | Target::Lifeline
                    | Target::Message
                    | Target::MessageLabel
                    | Target::LoopLabelBackground
                    | Target::LoopLabel
                    | Target::Activation
                    | Target::Note
                    | Target::NoteLabel
                    | Target::Title
            ) =>
            {
                DEFAULT
            }
            Stroke
                if matches!(
                    target,
                    Target::Actor
                        | Target::Lifeline
                        | Target::Message
                        | Target::LoopLabelBackground
                        | Target::Activation
                        | Target::Note
                ) =>
            {
                DEFAULT
            }
            _ => &[],
        },
        Family::GANTT => match (target, channel) {
            (Target::Text | Target::Title, Fill) => DEFAULT,
            (Target::Task, Fill) => GANTT_FILL,
            (Target::Task, Stroke) => GANTT_STROKE,
            _ => &[],
        },
        Family::KANBAN => match (target, channel) {
            (Target::Text | Target::Title, Fill) => DEFAULT,
            (Target::Task, Stroke) => DEFAULT,
            _ => &[],
        },
        Family::REQUIREMENT => match (target, channel) {
            (Target::Requirement | Target::Relation, Fill | Stroke) => DEFAULT,
            (Target::Text, Fill) => DEFAULT,
            _ => &[],
        },
        Family::ER => match (target, channel) {
            (Target::Relation, Fill | Stroke) => DEFAULT,
            (Target::Text, Fill) => DEFAULT,
            (Target::Table, Fill) => ODD_EVEN,
            _ => &[],
        },
        Family::PIE => match (target, channel) {
            (Target::Text | Target::Title | Target::PieSlice, Fill) => DEFAULT,
            (Target::PieSlice, Stroke) => DEFAULT,
            _ => &[],
        },
        Family::XY_CHART | Family::QUADRANT_CHART => match (target, channel) {
            (Target::Text | Target::Title | Target::Axis, Fill) => DEFAULT,
            (Target::Axis, Stroke) => DEFAULT,
            _ => &[],
        },
        Family::RADAR => match (target, channel) {
            (Target::Text | Target::Axis, Fill) => DEFAULT,
            (Target::Axis, Stroke) => DEFAULT,
            (Target::Title, Fill) => &[Variant::Default],
            _ => &[],
        },
        Family::TIMELINE => match (target, channel) {
            (Target::TimelineEvent, Fill | Stroke) => DEFAULT,
            (Target::Text, Fill) => DEFAULT,
            _ => &[],
        },
        Family::JOURNEY => match (target, channel) {
            (Target::JourneyTask, Fill | Stroke) => DEFAULT,
            (Target::Text | Target::Title, Fill) => DEFAULT,
            _ => &[],
        },
        Family::INFO => match (target, channel) {
            (Target::Text, Fill) => DEFAULT,
            _ => &[],
        },
        Family::ERROR => &[],
        Family::ZENUML
        | Family::ARCHITECTURE
        | Family::C4
        | Family::CYNEFIN
        | Family::SANKEY
        | Family::TREEMAP
        | Family::ISHIKAWA
        | Family::EVENT_MODELING
        | Family::VENN => match (target, channel) {
            (Target::Text | Target::Title, Fill) => DEFAULT,
            _ => &[],
        },
        Family::PACKET | Family::STATE => &[],
        Family::MINDMAP
        | Family::FLOWCHART
        | Family::SWIMLANE
        | Family::CLASS
        | Family::BLOCK
        | Family::GIT_GRAPH
        | Family::TREE_VIEW => unreachable!("node families were handled above"),
        _ => &[],
    }
}

#[cfg(test)]
#[path = "family_mechanism_matrix/tests.rs"]
mod tests;
