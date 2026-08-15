use crate::DiagramFamilyId;
#[cfg(test)]
use crate::theme_route_cutover::ThemeRouteCutoverProjection;
#[cfg(any(test, feature = "internal-theme-acceptance"))]
use crate::theme_route_cutover::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverId,
    ThemeRouteCutoverInventoryError, ThemeRouteCutoverProjectionSet, ThemeRouteCutoverSelector,
    ThemeRouteCutoverValue,
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

/// Returns every currently typed route that changes ownership of legacy bridge projections.
///
/// Direct-only routes such as radius, dasharray, or Flowchart ordinal palettes are deliberately
/// absent because they never had an equivalent bridge projection to retire.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(crate) fn legacy_replacing_typed_routes()
-> Result<Vec<ThemeRouteCutoverDescriptor>, ThemeRouteCutoverInventoryError> {
    let mut routes = Vec::new();
    for &family in DiagramFamilyId::all() {
        for &target in ThemeTarget::ALL {
            for (facet, channel) in [
                (ThemeRouteCutoverFacet::Fill, PaintChannel::Fill),
                (ThemeRouteCutoverFacet::Stroke, PaintChannel::Stroke),
            ] {
                for (value, paint_kind) in [
                    (
                        ThemeRouteCutoverValue::Transparent,
                        FamilyThemePaintKind::Transparent,
                    ),
                    (ThemeRouteCutoverValue::Solid, FamilyThemePaintKind::Solid),
                ] {
                    let rule_facet = match facet {
                        ThemeRouteCutoverFacet::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
                        ThemeRouteCutoverFacet::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
                    };
                    let selector = FamilyThemeSelectorShape::Static { variant: None };
                    if classify_rule_facet(family, target, selector, rule_facet)
                        != FamilyThemeDisposition::TypedAdapter
                        || !legacy_paint_supported(family, target, None, channel)
                    {
                        continue;
                    }
                    let projections =
                        legacy_bridge_projections(family, target, facet).ok_or_else(|| {
                            ThemeRouteCutoverInventoryError::missing_projections(
                                family, target, facet,
                            )
                        })?;
                    routes.push(ThemeRouteCutoverDescriptor::new(
                        ThemeRouteCutoverId::new(
                            family,
                            target,
                            ThemeRouteCutoverSelector::StaticUnqualified,
                            facet,
                            value,
                        ),
                        projections,
                    ));
                }
            }
        }
    }
    routes.sort_unstable();
    Ok(routes)
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn legacy_bridge_projections(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeRouteCutoverFacet,
) -> Option<ThemeRouteCutoverProjectionSet> {
    match (family, target, facet) {
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_FILL),
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_STROKE),
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_STROKE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NOTE_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NOTE_STROKE)
        }
        _ => None,
    }
}

pub(super) fn compile_base_typography_routes(
    family: DiagramFamilyId,
    style: &TextStyle,
) -> Vec<FamilyThemeRoute> {
    let baseline = TextStyle::default();
    let mut routes = Vec::new();
    if style.font_stack() != baseline.font_stack() {
        let disposition = if family != DiagramFamilyId::STATE
            && style.font_stack().as_css().len() > MAX_LEGACY_ASSIGNMENT_STRING_BYTES
        {
            FamilyThemeDisposition::Unsupported
        } else {
            classify_base_typography(family, ThemeTypographyProperty::FontStack)
        };
        routes.push(FamilyThemeRoute::new(
            FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack),
            disposition,
        ));
    }
    if style.font_size_px() != baseline.font_size_px() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontSize);
    }
    if style.font_weight() != baseline.font_weight() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontWeight);
    }
    if style.font_style() != baseline.font_style() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::FontStyle);
    }
    if style.line_height() != baseline.line_height() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::LineHeight);
    }
    if style.letter_spacing_px() != baseline.letter_spacing_px() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::LetterSpacing);
    }
    if style.word_spacing_px() != baseline.word_spacing_px() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::WordSpacing);
    }
    if style.transform() != baseline.transform() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::Transform);
    }
    if style.decoration() != baseline.decoration() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::Decoration);
    }
    if style.text_align() != baseline.text_align() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::TextAlign);
    }
    if style.white_space() != baseline.white_space() {
        push_base_typography(&mut routes, family, ThemeTypographyProperty::WhiteSpace);
    }
    if style.wrap() != baseline.wrap() {
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
        if family == DiagramFamilyId::STATE
            || (matches!(
                family,
                DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
            ) && target == ThemeTarget::Node)
        {
            FamilyThemeDisposition::TypedAdapter
        } else if legacy_palette_supported(family, target) {
            FamilyThemeDisposition::LegacyCompatibility
        } else {
            FamilyThemeDisposition::Unsupported
        },
    )
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
        if family == DiagramFamilyId::STATE {
            FamilyThemeDisposition::TypedAdapter
        } else {
            FamilyThemeDisposition::Unsupported
        },
    )
}

fn classify_base_typography(
    family: DiagramFamilyId,
    property: ThemeTypographyProperty,
) -> FamilyThemeDisposition {
    if family == DiagramFamilyId::STATE {
        return FamilyThemeDisposition::TypedAdapter;
    }
    match property {
        ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize => {
            FamilyThemeDisposition::LegacyCompatibility
        }
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

fn classify_rule_facet(
    family: DiagramFamilyId,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> FamilyThemeDisposition {
    if family == DiagramFamilyId::STATE {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && matches!(target, ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel)
        && matches!(
            selector,
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default)
            }
        )
        && (matches!(
            facet,
            FamilyThemeRuleFacet::Typography(
                ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize
            )
        ) || (target == ThemeTarget::EdgeLabel && facet == FamilyThemeRuleFacet::Padding))
    {
        return FamilyThemeDisposition::TypedAdapter;
    }
    if family == DiagramFamilyId::SEQUENCE
        && matches!(target, ThemeTarget::Actor | ThemeTarget::Note)
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
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Edge
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
    if matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
    ) && target == ThemeTarget::Node
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

    let node_family = matches!(
        family,
        Family::FLOWCHART
            | Family::SWIMLANE
            | Family::CLASS
            | Family::MINDMAP
            | Family::TREE_VIEW
            | Family::BLOCK
            | Family::GIT_GRAPH
    );
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
                ) || (family == Family::GIT_GRAPH && target == Target::EdgeLabel)
            }
            Stroke => matches!(
                target,
                Target::Node | Target::Edge | Target::Marker | Target::Cluster
            ),
        };
        if common {
            return DEFAULT;
        }
        if family == Family::CLASS && target == Target::Table && channel == Fill {
            return ODD_EVEN;
        }
        return &[];
    }

    match family {
        Family::SEQUENCE => match channel {
            Fill if matches!(
                target,
                Target::Actor
                    | Target::ActorLabel
                    | Target::Text
                    | Target::Lifeline
                    | Target::Message
                    | Target::MessageLabel
                    | Target::Loop
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
                        | Target::Loop
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
            (Target::Table, Fill) => ODD_EVEN,
            _ => &[],
        },
        Family::ER => match (target, channel) {
            (Target::Requirement | Target::Relation, Fill | Stroke) => DEFAULT,
            (Target::Text | Target::Title, Fill) => DEFAULT,
            (Target::Table, Fill) => ODD_EVEN,
            _ => &[],
        },
        Family::PIE => match (target, channel) {
            (Target::Text | Target::Title | Target::PieSlice, Fill) => DEFAULT,
            (Target::PieSlice, Stroke) => DEFAULT,
            _ => &[],
        },
        Family::XY_CHART | Family::QUADRANT_CHART | Family::RADAR => match (target, channel) {
            (Target::Text | Target::Title | Target::Axis, Fill) => DEFAULT,
            (Target::Axis, Stroke) => DEFAULT,
            _ => &[],
        },
        Family::TIMELINE => match (target, channel) {
            (Target::TimelineEvent, Fill | Stroke) => DEFAULT,
            (Target::Text | Target::Title, Fill) => DEFAULT,
            _ => &[],
        },
        Family::JOURNEY => match (target, channel) {
            (Target::JourneyTask, Fill | Stroke) => DEFAULT,
            (Target::Text | Target::Title, Fill) => DEFAULT,
            _ => &[],
        },
        Family::ERROR
        | Family::ZENUML
        | Family::ARCHITECTURE
        | Family::C4
        | Family::CYNEFIN
        | Family::WARDLEY
        | Family::RAILROAD
        | Family::SANKEY
        | Family::INFO
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

fn legacy_palette_supported(family: DiagramFamilyId, target: ThemeTarget) -> bool {
    match family {
        DiagramFamilyId::MINDMAP | DiagramFamilyId::GIT_GRAPH => target == ThemeTarget::Node,
        DiagramFamilyId::KANBAN => target == ThemeTarget::Task,
        DiagramFamilyId::PIE => target == ThemeTarget::PieSlice,
        DiagramFamilyId::XY_CHART | DiagramFamilyId::RADAR => target == ThemeTarget::ChartSeries,
        DiagramFamilyId::TIMELINE => target == ThemeTarget::TimelineEvent,
        DiagramFamilyId::JOURNEY => target == ThemeTarget::JourneyTask,
        DiagramFamilyId::ERROR
        | DiagramFamilyId::STATE
        | DiagramFamilyId::SEQUENCE
        | DiagramFamilyId::ZENUML
        | DiagramFamilyId::FLOWCHART
        | DiagramFamilyId::SWIMLANE
        | DiagramFamilyId::ARCHITECTURE
        | DiagramFamilyId::CLASS
        | DiagramFamilyId::C4
        | DiagramFamilyId::CYNEFIN
        | DiagramFamilyId::WARDLEY
        | DiagramFamilyId::RAILROAD
        | DiagramFamilyId::GANTT
        | DiagramFamilyId::PACKET
        | DiagramFamilyId::REQUIREMENT
        | DiagramFamilyId::SANKEY
        | DiagramFamilyId::INFO
        | DiagramFamilyId::TREEMAP
        | DiagramFamilyId::BLOCK
        | DiagramFamilyId::ER
        | DiagramFamilyId::QUADRANT_CHART
        | DiagramFamilyId::TREE_VIEW
        | DiagramFamilyId::ISHIKAWA
        | DiagramFamilyId::EVENT_MODELING
        | DiagramFamilyId::VENN => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        TextStylePatch, ThemeColorValue, ThemeGeometryPatch, ThemePaintPatch, ThemeStylePatch,
    };

    #[test]
    fn mixed_flowchart_rule_is_split_by_facet() {
        let rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                paint: ThemePaintPatch {
                    fill: Specified::Value(
                        CanvasPaint::solid("#abcdef").expect("valid fixture color"),
                    ),
                    ..ThemePaintPatch::default()
                },
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(8.0),
                },
                ..ThemeStylePatch::default()
            },
        );
        let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 7, &rule);

        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::RuleFacet {
                    rule_index: 7,
                    target: ThemeTarget::Node,
                    selector: FamilyThemeSelectorShape::Static { variant: None },
                    facet: FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
                }
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
        assert!(routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    facet: FamilyThemeRuleFacet::Radius,
                    ..
                }
            ) && route.disposition() == FamilyThemeDisposition::TypedAdapter
        }));
    }

    #[test]
    fn flowchart_and_swimlane_keep_explicit_default_node_paint_in_compatibility() {
        let default_style = ThemeStylePatch {
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(8.0),
            },
            ..ThemeStylePatch::default()
        }
        .with_fill(CanvasPaint::Transparent)
        .with_stroke(CanvasPaint::solid("#abcdef").expect("valid fixture color"))
        .with_stroke_width(2.5)
        .expect("valid fixture stroke width")
        .with_stroke_dasharray([4.0, 2.0])
        .expect("valid fixture stroke dasharray");
        let default_rule =
            ThemeRule::new(ThemeTarget::Node, default_style).with_variant(ThemeVariant::Default);
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            let routes = compile_rule_routes(family, 0, &default_rule);
            for route in routes {
                let expected = match route.mechanism() {
                    FamilyThemeMechanism::RuleFacet {
                        facet: FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_),
                        ..
                    } => FamilyThemeDisposition::LegacyCompatibility,
                    FamilyThemeMechanism::RuleFacet {
                        facet:
                            FamilyThemeRuleFacet::StrokeWidth
                            | FamilyThemeRuleFacet::StrokeDasharray
                            | FamilyThemeRuleFacet::Radius,
                        ..
                    } => FamilyThemeDisposition::TypedAdapter,
                    mechanism => panic!("unexpected route {mechanism:?}"),
                };
                assert_eq!(route.disposition(), expected);
            }
            assert_eq!(
                compile_ordinal_palette_route(family, ThemeTarget::Node).disposition(),
                FamilyThemeDisposition::TypedAdapter
            );
            assert_eq!(
                compile_ordinal_palette_route(family, ThemeTarget::Edge).disposition(),
                FamilyThemeDisposition::Unsupported
            );
        }

        let active_rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
        )
        .with_variant(ThemeVariant::Active);
        assert_eq!(
            compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &active_rule)[0].disposition(),
            FamilyThemeDisposition::Unsupported
        );

        let edge_geometry_rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke_width(2.5)
                .expect("valid edge width")
                .with_stroke_dasharray([4.0, 2.0])
                .expect("valid edge dasharray"),
        );
        assert!(
            compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &edge_geometry_rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );

        let ordinal_geometry_rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_stroke_width(2.5)
                .expect("valid ordinal width")
                .with_stroke_dasharray([4.0, 2.0])
                .expect("valid ordinal dasharray"),
        )
        .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap());
        assert!(
            compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &ordinal_geometry_rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );

        for rule in [
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch {
                        radius: Specified::Value(8.0),
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .with_variant(ThemeVariant::Active),
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch {
                        radius: Specified::Value(8.0),
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
            ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch {
                    geometry: ThemeGeometryPatch {
                        radius: Specified::Value(8.0),
                    },
                    ..ThemeStylePatch::default()
                },
            ),
        ] {
            assert_eq!(
                compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule)[0].disposition(),
                FamilyThemeDisposition::Unsupported
            );
        }
    }

    #[test]
    fn flowchart_and_swimlane_only_own_unqualified_scalar_edge_stroke() {
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for paint in [
                    CanvasPaint::Transparent,
                    CanvasPaint::solid("#abcdef").expect("valid fixture color"),
                ] {
                    let mut rule = ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(paint),
                    );
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let routes = compile_rule_routes(family, 0, &rule);
                    assert_eq!(routes.len(), 1);
                    assert_eq!(
                        routes[0].disposition(),
                        if variant.is_none() {
                            FamilyThemeDisposition::TypedAdapter
                        } else {
                            FamilyThemeDisposition::LegacyCompatibility
                        }
                    );
                }
            }

            let legacy_fill = ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
            );
            assert_eq!(
                compile_rule_routes(family, 0, &legacy_fill)[0].disposition(),
                FamilyThemeDisposition::LegacyCompatibility
            );

            for rule in [
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default()
                        .with_stroke(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
                )
                .with_variant(ThemeVariant::Active),
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default()
                        .with_stroke(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
                )
                .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
            ] {
                assert_eq!(
                    compile_rule_routes(family, 0, &rule)[0].disposition(),
                    FamilyThemeDisposition::Unsupported
                );
            }
            assert_eq!(
                compile_ordinal_palette_route(family, ThemeTarget::Edge).disposition(),
                FamilyThemeDisposition::Unsupported
            );
        }
    }

    #[test]
    fn flowchart_and_swimlane_own_static_default_label_typography_and_edge_padding() {
        let font_stack = super::super::FontStack::single("Excalifont").expect("valid font stack");
        let style = ThemeStylePatch {
            typography: TextStylePatch {
                font_stack: Specified::Value(font_stack),
                font_size_px: Specified::Value(20.0),
                ..TextStylePatch::default()
            },
            ..ThemeStylePatch::default()
        };
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            for target in [ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel] {
                let unqualified_rule = ThemeRule::new(target, style.clone());
                assert!(
                    compile_rule_routes(family, 0, &unqualified_rule)
                        .iter()
                        .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
                );

                let default_rule =
                    ThemeRule::new(target, style.clone()).with_variant(ThemeVariant::Default);
                assert!(
                    compile_rule_routes(family, 0, &default_rule)
                        .iter()
                        .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
                );

                for rule in [
                    ThemeRule::new(target, style.clone()).with_variant(ThemeVariant::Active),
                    ThemeRule::new(target, style.clone())
                        .with_ordinal(crate::diagram_theme::OrdinalSelector::exact(1).unwrap()),
                ] {
                    assert!(compile_rule_routes(family, 0, &rule).iter().all(|route| {
                        route.disposition() == FamilyThemeDisposition::Unsupported
                    }));
                }

                let font_weight_rule = ThemeRule::new(
                    target,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_weight: Specified::Value(700),
                            ..TextStylePatch::default()
                        },
                        ..ThemeStylePatch::default()
                    },
                );
                assert!(
                    compile_rule_routes(family, 0, &font_weight_rule)
                        .iter()
                        .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
                );
            }

            assert!(
                compile_rule_routes(family, 0, &ThemeRule::new(ThemeTarget::Edge, style.clone()))
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
            );

            let edge_padding = ThemeRule::new(
                ThemeTarget::EdgeLabel,
                ThemeStylePatch::default().with_padding(super::super::InsetsPx::all(8.0)),
            );
            assert!(
                compile_rule_routes(family, 0, &edge_padding)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
            );
            let node_padding = ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_padding(super::super::InsetsPx::all(8.0)),
            );
            assert!(
                compile_rule_routes(family, 0, &node_padding)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
            );
        }
    }

    #[test]
    fn gradient_and_rule_typography_do_not_enter_the_legacy_bridge() {
        let gradient = super::super::canvas::LinearGradient::new(
            90.0,
            [
                super::super::canvas::GradientStop::new(
                    0.0,
                    ThemeColorValue::parse("#000000").expect("valid color"),
                )
                .expect("valid stop"),
                super::super::canvas::GradientStop::new(
                    1.0,
                    ThemeColorValue::parse("#ffffff").expect("valid color"),
                )
                .expect("valid stop"),
            ],
        )
        .expect("valid gradient");
        let mut typography = TextStylePatch::default();
        typography.font_weight = Specified::Value(700);
        let rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                paint: ThemePaintPatch {
                    fill: Specified::Value(CanvasPaint::LinearGradient(gradient)),
                    ..ThemePaintPatch::default()
                },
                typography,
                ..ThemeStylePatch::default()
            },
        );

        let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule);

        assert_eq!(routes.len(), 2);
        assert!(
            routes
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::Unsupported)
        );
    }

    #[test]
    fn state_owns_every_applicable_rule_facet() {
        let rule = ThemeRule::new(
            ThemeTarget::State,
            ThemeStylePatch {
                paint: ThemePaintPatch {
                    fill: Specified::Clear,
                    ..ThemePaintPatch::default()
                },
                geometry: ThemeGeometryPatch {
                    radius: Specified::Value(4.0),
                },
                ..ThemeStylePatch::default()
            },
        );

        assert!(
            compile_rule_routes(DiagramFamilyId::STATE, 0, &rule)
                .iter()
                .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
        );
    }

    #[test]
    fn ordinal_rules_are_not_silently_projected_by_the_legacy_bridge() {
        let rule = ThemeRule::new(
            ThemeTarget::Actor,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").expect("valid fixture color")),
        )
        .with_ordinal(OrdinalSelector::exact(1).expect("valid ordinal"));
        let routes = compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &rule);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].disposition(), FamilyThemeDisposition::Unsupported);
    }

    #[test]
    fn sequence_actor_and_note_direct_routes_only_accept_static_scalar_paints() {
        let gradient = super::super::canvas::LinearGradient::new(
            90.0,
            [
                super::super::canvas::GradientStop::new(
                    0.0,
                    ThemeColorValue::parse("#000000").expect("valid color"),
                )
                .expect("valid stop"),
                super::super::canvas::GradientStop::new(
                    1.0,
                    ThemeColorValue::parse("#ffffff").expect("valid color"),
                )
                .expect("valid stop"),
            ],
        )
        .expect("valid gradient");
        for target in [ThemeTarget::Actor, ThemeTarget::Note] {
            let solid = ThemeRule::new(
                target,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#abcdef").expect("valid fill"))
                    .with_stroke(CanvasPaint::solid("#123456").expect("valid stroke")),
            );
            let gradient_rule = ThemeRule::new(
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient.clone())),
            );

            let solid_routes = compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &solid);
            assert_eq!(solid_routes.len(), 2);
            assert!(
                solid_routes
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::TypedAdapter)
            );
            let explicit_default = ThemeRule::new(
                target,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#abcdef").expect("valid fill"))
                    .with_stroke(CanvasPaint::solid("#123456").expect("valid stroke")),
            )
            .with_variant(ThemeVariant::Default);
            assert!(
                compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &explicit_default)
                    .iter()
                    .all(|route| route.disposition() == FamilyThemeDisposition::LegacyCompatibility)
            );
            assert_eq!(
                compile_rule_routes(DiagramFamilyId::SEQUENCE, 0, &gradient_rule)[0].disposition(),
                FamilyThemeDisposition::Unsupported
            );
        }
    }

    #[test]
    fn legacy_replacing_typed_routes_are_derived_from_the_matrix() {
        use ThemeRouteCutoverFacet::{Fill, Stroke};
        use ThemeRouteCutoverValue::{Solid, Transparent};

        let actual = legacy_replacing_typed_routes()
            .expect("derive typed legacy-replacing routes")
            .into_iter()
            .map(|route| {
                (
                    route.family_id(),
                    route.target(),
                    route.facet(),
                    route.value(),
                    route
                        .projections()
                        .iter()
                        .map(ThemeRouteCutoverProjection::contribution_id)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let expected = vec![
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                Fill,
                Transparent,
                vec!["node.fill"],
            ),
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                Fill,
                Solid,
                vec!["node.fill"],
            ),
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                Stroke,
                Transparent,
                vec!["node.stroke"],
            ),
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                Stroke,
                Solid,
                vec!["node.stroke"],
            ),
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Edge,
                Stroke,
                Transparent,
                vec!["edge.stroke", "marker.paint-from-edge"],
            ),
            (
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Edge,
                Stroke,
                Solid,
                vec!["edge.stroke", "marker.paint-from-edge"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Actor,
                Fill,
                Transparent,
                vec!["actor.fill"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Actor,
                Fill,
                Solid,
                vec!["actor.fill"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Actor,
                Stroke,
                Transparent,
                vec!["actor.stroke"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Actor,
                Stroke,
                Solid,
                vec!["actor.stroke"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Note,
                Fill,
                Transparent,
                vec!["note.fill"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Note,
                Fill,
                Solid,
                vec!["note.fill"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Note,
                Stroke,
                Transparent,
                vec!["note.stroke"],
            ),
            (
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Note,
                Stroke,
                Solid,
                vec!["note.stroke"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Node,
                Fill,
                Transparent,
                vec!["node.fill"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Node,
                Fill,
                Solid,
                vec!["node.fill"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Node,
                Stroke,
                Transparent,
                vec!["node.stroke"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Node,
                Stroke,
                Solid,
                vec!["node.stroke"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Edge,
                Stroke,
                Transparent,
                vec!["edge.stroke", "marker.paint-from-edge"],
            ),
            (
                DiagramFamilyId::SWIMLANE,
                ThemeTarget::Edge,
                Stroke,
                Solid,
                vec!["edge.stroke", "marker.paint-from-edge"],
            ),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn legacy_replacing_typed_route_ids_are_unique() {
        let routes = legacy_replacing_typed_routes().expect("derive typed legacy-replacing routes");
        let ids = routes
            .iter()
            .map(|route| route.id())
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(ids.len(), routes.len());
    }

    #[test]
    fn edge_cutover_replaces_stroke_and_retires_marker_fallback() {
        let route = legacy_replacing_typed_routes()
            .expect("derive typed legacy-replacing routes")
            .into_iter()
            .find(|route| {
                route.family_id() == DiagramFamilyId::FLOWCHART
                    && route.target() == ThemeTarget::Edge
                    && route.facet() == ThemeRouteCutoverFacet::Stroke
                    && route.value() == ThemeRouteCutoverValue::Solid
            })
            .expect("Flowchart Edge.stroke solid route");
        let projections = route.projections().iter().collect::<Vec<_>>();

        assert_eq!(
            projections,
            vec![
                ThemeRouteCutoverProjection::EdgeStroke,
                ThemeRouteCutoverProjection::MarkerPaintFromEdge,
            ]
        );
        assert_eq!(
            projections[0].action(),
            crate::theme_route_cutover::ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(
            projections[1].action(),
            crate::theme_route_cutover::ThemeRouteCutoverProjectionAction::RetireFallback
        );
    }

    #[test]
    fn explicit_clear_is_not_treated_as_a_legacy_paint() {
        let rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                paint: ThemePaintPatch {
                    fill: Specified::Clear,
                    ..ThemePaintPatch::default()
                },
                ..ThemeStylePatch::default()
            },
        );

        let routes = compile_rule_routes(DiagramFamilyId::FLOWCHART, 0, &rule);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].disposition(), FamilyThemeDisposition::Unsupported);
        assert!(matches!(
            routes[0].mechanism(),
            FamilyThemeMechanism::RuleFacet {
                facet: FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Clear),
                ..
            }
        ));
    }

    #[test]
    fn only_font_stack_and_size_are_legacy_base_typography() {
        let typography = TextStyle::default()
            .with_font_size_px(18.0)
            .expect("valid font size")
            .with_font_weight(700)
            .expect("valid font weight");
        let routes = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &typography);

        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::LegacyCompatibility
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
                && route.disposition() == FamilyThemeDisposition::Unsupported
        }));
    }

    #[test]
    fn flowchart_and_swimlane_base_typography_remains_legacy_until_shared_layout_plan() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::single("Excalifont").expect("valid fixture font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size")
            .with_font_weight(700)
            .expect("valid font weight");

        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            let routes = compile_base_typography_routes(family, &typography);

            assert!(routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    && route.disposition() == FamilyThemeDisposition::LegacyCompatibility
            }));
            assert!(routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::LegacyCompatibility
            }));
            assert!(routes.iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontWeight)
                    && route.disposition() == FamilyThemeDisposition::Unsupported
            }));
        }
    }

    #[test]
    fn oversized_font_stack_is_unsupported_without_shadowing_font_size() {
        let families = (0..32)
            .map(|index| format!("font-{index}-{}", "x".repeat(180)))
            .collect::<Vec<_>>();
        let typography = TextStyle::default()
            .with_font_stack(super::super::FontStack::new(families).expect("valid font stack"))
            .with_font_size_px(18.0)
            .expect("valid font size");
        let routes = compile_base_typography_routes(DiagramFamilyId::SEQUENCE, &typography);

        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                && route.disposition() == FamilyThemeDisposition::Unsupported
        }));
        assert!(routes.iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::LegacyCompatibility
        }));
    }
}
