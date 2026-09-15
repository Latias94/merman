use merman_theme_contract::{
    THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCapabilityDescriptorV1, ThemeRuleFacetV1,
    ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeSupportSubjectV1,
};

use crate::DiagramFamilyId;

use super::semantic::ThemeTarget;
use super::support_manifest::{self, SupportClaimKind};

const THEME_SUPPORT_CLAIM_REVISION: u32 = super::support_manifest::SUPPORT_CLAIM_MANIFEST_REVISION;

/// Describes static support for a query subject.
///
/// A support claim is not proof of document execution or portable output.
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

    #[derive(Clone, Copy)]
    enum ResolvedSubject {
        Target {
            target: ThemeTarget,
            facet: ThemeSupportFacetV1,
        },
        BaseTypography(ThemeSupportBaseTypographyPropertyV1),
    }

    let subject = match query.subject() {
        ThemeSupportSubjectV1::Rule { target, facet } => {
            let Some(target) = ThemeTarget::from_id(target) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-target"],
                );
            };
            let Some(facet) = ThemeRuleFacetV1::from_id(facet) else {
                return descriptor(
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
        ThemeSupportSubjectV1::OrdinalPalette { target } => {
            let Some(target) = ThemeTarget::from_id(target) else {
                return descriptor(
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
        ThemeSupportSubjectV1::BaseTypography { property } => {
            let Some(property) = ThemeSupportBaseTypographyPropertyV1::from_id(property) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-base-typography-property"],
                );
            };
            ResolvedSubject::BaseTypography(property)
        }
        ThemeSupportSubjectV1::Unknown(_) => {
            return descriptor(
                query,
                ThemeSupportStateV1::Unverified,
                ["theme-support.unknown-subject"],
            );
        }
        _ => {
            return descriptor(
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
        return descriptor(query, state, [reason_id]);
    }

    let claim = match subject {
        ResolvedSubject::Target { target, facet } => target_claim(family, target, facet),
        ResolvedSubject::BaseTypography(property) => {
            support_manifest::base_typography_claim(family, property)
        }
    };
    descriptor_from_claim(query, claim)
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
    if let Some(target) = target
        && !target.valid_for(family)
    {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::NotApplicable,
            reason_id: "theme-support.target-not-applicable-to-family",
        });
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
