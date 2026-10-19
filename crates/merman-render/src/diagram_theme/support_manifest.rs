//! Versioned public support projection of the family mechanism matrix.

use super::family_mechanism_matrix::{
    FamilyThemeSupportSummary, summarize_base_typography_support, summarize_theme_support,
};
use super::semantic::ThemeTarget;
use crate::DiagramFamilyId;
use merman_theme_contract::{
    ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1,
};

/// Revision of the published support projection, independent of private route representation.
pub(super) const SUPPORT_CLAIM_MANIFEST_REVISION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SupportClaimKind {
    TypedSurface,
    TypedPartial,
    Unsupported,
    Missing,
}

pub(super) fn rule_claim(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeRuleFacetV1,
) -> SupportClaimKind {
    claim_kind(summarize_theme_support(
        family,
        target,
        ThemeSupportFacetV1::Rule(facet),
    ))
}

pub(super) fn ordinal_claim(family: DiagramFamilyId, target: ThemeTarget) -> SupportClaimKind {
    claim_kind(summarize_theme_support(
        family,
        target,
        ThemeSupportFacetV1::OrdinalPalette,
    ))
}

pub(super) fn base_typography_claim(
    family: DiagramFamilyId,
    property: ThemeSupportBaseTypographyPropertyV1,
) -> SupportClaimKind {
    claim_kind(summarize_base_typography_support(family, property))
}

fn claim_kind(summary: FamilyThemeSupportSummary) -> SupportClaimKind {
    match (summary.has_typed(), summary.has_unsupported()) {
        (true, false) => SupportClaimKind::TypedSurface,
        (true, true) => SupportClaimKind::TypedPartial,
        (false, true) => SupportClaimKind::Unsupported,
        (false, false) => SupportClaimKind::Missing,
    }
}
