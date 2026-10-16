//! Selector-domain enumeration shared by support discovery and route invariants.

use super::*;

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
