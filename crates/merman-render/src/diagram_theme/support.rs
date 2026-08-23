use merman_theme_contract::{
    THEME_SUPPORT_SCHEMA_VERSION_V1, THEME_SUPPORT_SCHEMA_VERSION_V2, ThemeCapabilityDescriptorV1,
    ThemeCapabilityDescriptorV2, ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV2,
    ThemeSupportFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1, ThemeSupportQueryV2,
    ThemeSupportStateV1, ThemeSupportSubjectV2,
};

use crate::DiagramFamilyId;

use super::semantic::ThemeTarget;
use super::support_manifest::{self, SupportClaimKind};

const THEME_SUPPORT_CLAIM_REVISION_V1: u32 =
    super::support_manifest::SUPPORT_CLAIM_MANIFEST_REVISION;
const THEME_SUPPORT_CLAIM_REVISION_V2: u32 =
    super::support_manifest::SUPPORT_CLAIM_MANIFEST_REVISION;

/// Describes the current build's coarse static support for one theme capability.
///
/// The result never claims that a concrete document applied the capability or that an output is
/// portable. Those facts remain owned by render evidence and target admission.
pub fn describe_theme_support(query: &ThemeSupportQueryV1) -> ThemeCapabilityDescriptorV1 {
    if query.schema_version() != THEME_SUPPORT_SCHEMA_VERSION_V1 {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-schema-version"],
        );
    }
    let Some(family) = DiagramFamilyId::from_id(query.family_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-family"],
        );
    };
    let Some(output) = ThemeSupportOutputV1::from_id(query.output_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-output"],
        );
    };
    let Some(target) = ThemeTarget::from_id(query.target_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-target"],
        );
    };
    let Some(facet) = ThemeSupportFacetV1::from_id(query.facet_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-facet"],
        );
    };
    if !target.valid_for(family) {
        return descriptor(
            query,
            ThemeSupportStateV1::NotApplicable,
            ["theme-support.target-not-applicable-to-family"],
        );
    }
    if output == ThemeSupportOutputV1::Ascii {
        return descriptor(
            query,
            ThemeSupportStateV1::NotApplicable,
            ["theme-support.visual-theme-not-applicable-to-output"],
        );
    }
    if output == ThemeSupportOutputV1::BrowserSvg {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.terminal-qualification-incomplete"],
        );
    }
    if matches!(
        output,
        ThemeSupportOutputV1::Png | ThemeSupportOutputV1::Jpeg | ThemeSupportOutputV1::Pdf
    ) {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.output-qualification-not-owned-by-renderer"],
        );
    }
    if target == ThemeTarget::Canvas {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.root-support-query-not-yet-modeled"],
        );
    }
    if family == DiagramFamilyId::ARCHITECTURE && !crate::layout_cytoscape_available() {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.family-render-capability-not-built"],
        );
    }

    let claim = match facet {
        ThemeSupportFacetV1::Rule(facet) => support_manifest::rule_claim(family, target, facet),
        ThemeSupportFacetV1::OrdinalPalette => support_manifest::ordinal_claim(family, target),
        _ => SupportClaimKind::Missing,
    };
    descriptor_from_claim(query, claim)
}

/// Describes the current build's coarse static support for one unstable alpha V2 subject.
///
/// V2 separates family-wide base typography from semantic-target rules. Output qualification and
/// family render-capability gates remain identical to V1. Its subject inventory and identifiers
/// remain unfrozen until the C7a rollout gate closes.
pub fn describe_theme_support_v2(query: &ThemeSupportQueryV2) -> ThemeCapabilityDescriptorV2 {
    if query.schema_version() != THEME_SUPPORT_SCHEMA_VERSION_V2 {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-schema-version"],
        );
    }
    let Some(family) = DiagramFamilyId::from_id(query.family_id()) else {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-family"],
        );
    };
    let Some(output) = ThemeSupportOutputV1::from_id(query.output_id()) else {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-output"],
        );
    };

    #[derive(Clone, Copy)]
    enum ResolvedSubject {
        Target {
            target: ThemeTarget,
            facet: ThemeSupportFacetV1,
        },
        BaseTypography(ThemeSupportBaseTypographyPropertyV2),
    }

    let subject = match query.subject() {
        ThemeSupportSubjectV2::Rule { target, facet } => {
            let Some(target) = ThemeTarget::from_id(target) else {
                return descriptor_v2(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-target"],
                );
            };
            let Some(facet) = ThemeRuleFacetV1::from_id(facet) else {
                return descriptor_v2(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-facet"],
                );
            };
            ResolvedSubject::Target {
                target,
                facet: ThemeSupportFacetV1::Rule(facet),
            }
        }
        ThemeSupportSubjectV2::OrdinalPalette { target } => {
            let Some(target) = ThemeTarget::from_id(target) else {
                return descriptor_v2(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-target"],
                );
            };
            ResolvedSubject::Target {
                target,
                facet: ThemeSupportFacetV1::OrdinalPalette,
            }
        }
        ThemeSupportSubjectV2::BaseTypography { property } => {
            let Some(property) = ThemeSupportBaseTypographyPropertyV2::from_id(property) else {
                return descriptor_v2(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-base-typography-property"],
                );
            };
            ResolvedSubject::BaseTypography(property)
        }
        ThemeSupportSubjectV2::Unknown(_) => {
            return descriptor_v2(
                query,
                ThemeSupportStateV1::Unverified,
                ["theme-support.unknown-subject"],
            );
        }
        _ => {
            return descriptor_v2(
                query,
                ThemeSupportStateV1::Unverified,
                ["theme-support.unknown-subject"],
            );
        }
    };

    if let ResolvedSubject::Target { target, .. } = subject {
        if !target.valid_for(family) {
            return descriptor_v2(
                query,
                ThemeSupportStateV1::NotApplicable,
                ["theme-support.target-not-applicable-to-family"],
            );
        }
    }
    if output == ThemeSupportOutputV1::Ascii {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::NotApplicable,
            ["theme-support.visual-theme-not-applicable-to-output"],
        );
    }
    if output == ThemeSupportOutputV1::BrowserSvg {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.terminal-qualification-incomplete"],
        );
    }
    if matches!(
        output,
        ThemeSupportOutputV1::Png | ThemeSupportOutputV1::Jpeg | ThemeSupportOutputV1::Pdf
    ) {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.output-qualification-not-owned-by-renderer"],
        );
    }
    if matches!(
        subject,
        ResolvedSubject::Target {
            target: ThemeTarget::Canvas,
            ..
        }
    ) {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.root-support-query-not-yet-modeled"],
        );
    }
    if family == DiagramFamilyId::ARCHITECTURE && !crate::layout_cytoscape_available() {
        return descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.family-render-capability-not-built"],
        );
    }

    let claim = match subject {
        ResolvedSubject::Target { target, facet } => match facet {
            ThemeSupportFacetV1::Rule(facet) => support_manifest::rule_claim(family, target, facet),
            ThemeSupportFacetV1::OrdinalPalette => support_manifest::ordinal_claim(family, target),
            _ => SupportClaimKind::Missing,
        },
        ResolvedSubject::BaseTypography(property) => {
            support_manifest::base_typography_claim(family, property)
        }
    };
    descriptor_v2_from_claim(query, claim)
}

fn descriptor_from_claim(
    query: &ThemeSupportQueryV1,
    claim: SupportClaimKind,
) -> ThemeCapabilityDescriptorV1 {
    match claim {
        SupportClaimKind::TypedSurface => descriptor(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
        ),
        SupportClaimKind::TypedPartial => descriptor(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
        ),
        SupportClaimKind::LegacySurface => descriptor(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ],
        ),
        SupportClaimKind::LegacyPartial => descriptor(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.public-value-domain-partial",
            ],
        ),
        SupportClaimKind::Unsupported => descriptor(
            query,
            ThemeSupportStateV1::Unsupported,
            ["theme-support.no-supported-route"],
        ),
        SupportClaimKind::Missing => descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.support-claim-missing"],
        ),
    }
}

fn descriptor_v2_from_claim(
    query: &ThemeSupportQueryV2,
    claim: SupportClaimKind,
) -> ThemeCapabilityDescriptorV2 {
    match claim {
        SupportClaimKind::TypedSurface => descriptor_v2(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
        ),
        SupportClaimKind::TypedPartial => descriptor_v2(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
        ),
        SupportClaimKind::LegacySurface => descriptor_v2(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ],
        ),
        SupportClaimKind::LegacyPartial => descriptor_v2(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.public-value-domain-partial",
            ],
        ),
        SupportClaimKind::Unsupported => descriptor_v2(
            query,
            ThemeSupportStateV1::Unsupported,
            ["theme-support.no-supported-route"],
        ),
        SupportClaimKind::Missing => descriptor_v2(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.support-claim-missing"],
        ),
    }
}

fn descriptor<const N: usize>(
    query: &ThemeSupportQueryV1,
    state: ThemeSupportStateV1,
    reason_ids: [&'static str; N],
) -> ThemeCapabilityDescriptorV1 {
    ThemeCapabilityDescriptorV1::from_renderer_claim(
        THEME_SUPPORT_CLAIM_REVISION_V1,
        query.clone(),
        state,
        reason_ids,
    )
}

fn descriptor_v2<const N: usize>(
    query: &ThemeSupportQueryV2,
    state: ThemeSupportStateV1,
    reason_ids: [&'static str; N],
) -> ThemeCapabilityDescriptorV2 {
    ThemeCapabilityDescriptorV2::from_renderer_claim(
        THEME_SUPPORT_CLAIM_REVISION_V2,
        query.clone(),
        state,
        reason_ids,
    )
}
