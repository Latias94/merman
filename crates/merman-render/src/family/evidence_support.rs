use crate::diagram_theme::{FamilyThemeRuleFacet, ResolvedStyleProperty};

use super::FamilyThemeResidualReason;

pub(crate) const fn resolved_style_property_for_facet(
    facet: FamilyThemeRuleFacet,
) -> ResolvedStyleProperty {
    match facet {
        FamilyThemeRuleFacet::Fill(_) => ResolvedStyleProperty::Fill,
        FamilyThemeRuleFacet::Stroke(_) => ResolvedStyleProperty::Stroke,
        FamilyThemeRuleFacet::StrokeWidth => ResolvedStyleProperty::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray => ResolvedStyleProperty::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap => ResolvedStyleProperty::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin => ResolvedStyleProperty::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity => ResolvedStyleProperty::Opacity,
        FamilyThemeRuleFacet::FillOpacity => ResolvedStyleProperty::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity => ResolvedStyleProperty::StrokeOpacity,
        FamilyThemeRuleFacet::Radius => ResolvedStyleProperty::Radius,
        FamilyThemeRuleFacet::Padding => ResolvedStyleProperty::Padding,
        FamilyThemeRuleFacet::Typography(property) => ResolvedStyleProperty::Typography(property),
        FamilyThemeRuleFacet::Effect => ResolvedStyleProperty::Effect,
    }
}

pub(crate) const fn unsupported_residual_for_facet(
    facet: FamilyThemeRuleFacet,
) -> FamilyThemeResidualReason {
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
