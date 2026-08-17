use merman_theme_contract::{
    THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCapabilityDescriptorV1, ThemeSupportFacetV1,
    ThemeSupportOutputV1, ThemeSupportQueryV1, ThemeSupportStateV1,
};

use crate::DiagramFamilyId;

use super::family_mechanism_matrix::summarize_theme_support;
use super::semantic::ThemeTarget;

const THEME_SUPPORT_CLAIM_REVISION_V1: u32 = 1;

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

    let summary = summarize_theme_support(family, target, facet);
    if summary.has_typed() {
        if summary.has_legacy() || summary.has_unsupported() {
            return descriptor(
                query,
                ThemeSupportStateV1::Conditional,
                [
                    "theme-support.family-owned-consumer-present",
                    "theme-support.public-value-domain-partial",
                ],
            );
        }
        return descriptor(
            query,
            ThemeSupportStateV1::Conditional,
            [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
        );
    }
    if summary.has_legacy() {
        let reasons = if summary.has_unsupported() {
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.public-value-domain-partial",
            ]
        } else {
            [
                "theme-support.legacy-compatibility-only",
                "theme-support.document-surface-dependent",
            ]
        };
        return descriptor(query, ThemeSupportStateV1::Conditional, reasons);
    }
    if summary.has_unsupported() {
        return descriptor(
            query,
            ThemeSupportStateV1::Unsupported,
            ["theme-support.no-supported-route"],
        );
    }

    descriptor(
        query,
        ThemeSupportStateV1::Unverified,
        ["theme-support.support-claim-missing"],
    )
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
