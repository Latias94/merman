//! Current route invariants over the independently enumerated matrix domain.

use super::*;

#[cfg(test)]
pub(super) fn for_each_matrix_selector(mut visit: impl FnMut(FamilyThemeSelectorShape)) {
    for variant in std::iter::once(None).chain(ThemeVariant::ALL.iter().copied().map(Some)) {
        visit(FamilyThemeSelectorShape::Static { variant });
        visit(FamilyThemeSelectorShape::Ordinal {
            variant,
            selector: OrdinalSelector::Exact(1),
        });
        visit(FamilyThemeSelectorShape::Ordinal {
            variant,
            selector: OrdinalSelector::Cycle {
                period: 1,
                offset: 0,
            },
        });
    }
}

#[cfg(test)]
pub(super) fn for_each_matrix_rule_facet(mut visit: impl FnMut(FamilyThemeRuleFacet)) {
    for kind in FamilyThemePaintKind::ALL {
        visit(FamilyThemeRuleFacet::Fill(kind));
        visit(FamilyThemeRuleFacet::Stroke(kind));
    }
    for facet in [
        FamilyThemeRuleFacet::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity,
        FamilyThemeRuleFacet::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity,
        FamilyThemeRuleFacet::Radius,
        FamilyThemeRuleFacet::Padding,
        FamilyThemeRuleFacet::Effect,
    ] {
        visit(facet);
    }
    for &property in ThemeTypographyProperty::ALL {
        visit(FamilyThemeRuleFacet::Typography(property));
    }
}
