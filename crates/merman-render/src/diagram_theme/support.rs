use merman_theme_contract::{
    THEME_SUPPORT_SCHEMA_VERSION_V1, THEME_SUPPORT_SCHEMA_VERSION_V2, ThemeCapabilityDescriptorV1,
    ThemeCapabilityDescriptorV2, ThemeRuleFacetV1, ThemeSupportBaseTypographyPropertyV2,
    ThemeSupportFacetV1, ThemeSupportOutputV1, ThemeSupportQueryV1, ThemeSupportQueryV2,
    ThemeSupportStateV1, ThemeSupportSubjectV2,
};

use crate::DiagramFamilyId;

use super::semantic::ThemeTarget;
use super::support_manifest::{self, SupportClaimKind};

const THEME_SUPPORT_CLAIM_REVISION: u32 = super::support_manifest::SUPPORT_CLAIM_MANIFEST_REVISION;

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
    if let Err(rejection) = qualify_common_support(family, output, Some(target)) {
        let SupportRejection { state, reason_id } = rejection;
        return descriptor(query, state, [reason_id]);
    }

    let claim = target_claim(family, target, facet);
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

    let target = match subject {
        ResolvedSubject::Target { target, .. } => Some(target),
        ResolvedSubject::BaseTypography(_) => None,
    };
    if let Err(rejection) = qualify_common_support(family, output, target) {
        let SupportRejection { state, reason_id } = rejection;
        return descriptor_v2(query, state, [reason_id]);
    }

    let claim = match subject {
        ResolvedSubject::Target { target, facet } => target_claim(family, target, facet),
        ResolvedSubject::BaseTypography(property) => {
            support_manifest::base_typography_claim(family, property)
        }
    };
    descriptor_v2_from_claim(query, claim)
}

fn target_claim(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeSupportFacetV1,
) -> SupportClaimKind {
    match facet {
        ThemeSupportFacetV1::Rule(facet) => support_manifest::rule_claim(family, target, facet),
        ThemeSupportFacetV1::OrdinalPalette => support_manifest::ordinal_claim(family, target),
        _ => SupportClaimKind::Missing,
    }
}

struct SupportRejection {
    state: ThemeSupportStateV1,
    reason_id: &'static str,
}

fn qualify_common_support(
    family: DiagramFamilyId,
    output: ThemeSupportOutputV1,
    target: Option<ThemeTarget>,
) -> Result<(), SupportRejection> {
    if let Some(target) = target {
        if !target.valid_for(family) {
            return Err(SupportRejection {
                state: ThemeSupportStateV1::NotApplicable,
                reason_id: "theme-support.target-not-applicable-to-family",
            });
        }
    }
    match output {
        ThemeSupportOutputV1::Ascii => {
            return Err(SupportRejection {
                state: ThemeSupportStateV1::NotApplicable,
                reason_id: "theme-support.visual-theme-not-applicable-to-output",
            });
        }
        ThemeSupportOutputV1::BrowserSvg => {
            return Err(SupportRejection {
                state: ThemeSupportStateV1::Unverified,
                reason_id: "theme-support.terminal-qualification-incomplete",
            });
        }
        ThemeSupportOutputV1::StandaloneSvg => {}
        _ => {
            // Unknown future output identifiers stay fail-closed until the renderer owns their
            // terminal qualification and the support manifest has an explicit row.
            return Err(SupportRejection {
                state: ThemeSupportStateV1::Unverified,
                reason_id: "theme-support.output-qualification-not-owned-by-renderer",
            });
        }
    }
    if target == Some(ThemeTarget::Canvas) {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.root-support-query-not-yet-modeled",
        });
    }
    if family == DiagramFamilyId::ARCHITECTURE && !crate::layout_cytoscape_available() {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.family-render-capability-not-built",
        });
    }
    Ok(())
}

fn descriptor_from_claim(
    query: &ThemeSupportQueryV1,
    claim: SupportClaimKind,
) -> ThemeCapabilityDescriptorV1 {
    match project_claim(claim) {
        SupportClaimProjection::OneReason { state, reason_id } => {
            descriptor(query, state, [reason_id])
        }
        SupportClaimProjection::TwoReasons { state, reason_ids } => {
            descriptor(query, state, reason_ids)
        }
    }
}

fn descriptor_v2_from_claim(
    query: &ThemeSupportQueryV2,
    claim: SupportClaimKind,
) -> ThemeCapabilityDescriptorV2 {
    match project_claim(claim) {
        SupportClaimProjection::OneReason { state, reason_id } => {
            descriptor_v2(query, state, [reason_id])
        }
        SupportClaimProjection::TwoReasons { state, reason_ids } => {
            descriptor_v2(query, state, reason_ids)
        }
    }
}

enum SupportClaimProjection {
    OneReason {
        state: ThemeSupportStateV1,
        reason_id: &'static str,
    },
    TwoReasons {
        state: ThemeSupportStateV1,
        reason_ids: [&'static str; 2],
    },
}

fn project_claim(claim: SupportClaimKind) -> SupportClaimProjection {
    match claim {
        SupportClaimKind::TypedSurface => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
        },
        SupportClaimKind::TypedPartial => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
        },
        SupportClaimKind::LegacySurface => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ],
        },
        SupportClaimKind::LegacyPartial => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.legacy-compatibility-only",
                "theme-support.public-value-domain-partial",
            ],
        },
        SupportClaimKind::Unsupported => SupportClaimProjection::OneReason {
            state: ThemeSupportStateV1::Unsupported,
            reason_id: "theme-support.no-supported-route",
        },
        SupportClaimKind::Missing => SupportClaimProjection::OneReason {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.support-claim-missing",
        },
    }
}

fn descriptor<const N: usize>(
    query: &ThemeSupportQueryV1,
    state: ThemeSupportStateV1,
    reason_ids: [&'static str; N],
) -> ThemeCapabilityDescriptorV1 {
    ThemeCapabilityDescriptorV1::from_renderer_claim(
        THEME_SUPPORT_CLAIM_REVISION,
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
        THEME_SUPPORT_CLAIM_REVISION,
        query.clone(),
        state,
        reason_ids,
    )
}
