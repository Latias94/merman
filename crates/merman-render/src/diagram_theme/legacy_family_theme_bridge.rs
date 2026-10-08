//! Final retirement observations for the removed family compatibility bridge.
//!
//! This module exists only for tests and private acceptance. It has no overlay builder,
//! provider, dispatch, or parse harness. Canonical inventory digests remain stable so the
//! independent historical acceptance ledger can reject route reintroduction.
//!
//! The release-preflight KTD23 gate calls `authorize_legacy_projection_retirements`, which
//! compares these observations with 80 independently declared historical projection rows.
//! Remove this final snapshot only when that gate and its historical reconciliation contract
//! are deliberately retired together; an empty current matrix does not replace that oracle.

use std::collections::BTreeSet;
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use super::canvas::CanvasPaint;
use super::family_mechanism_matrix::{
    FamilyThemeDisposition, FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape,
    LegacyCompatibilityRouteKey, classify_rule_facet, legacy_compatibility_route_inventory,
};
use super::family_program::FamilyThemeProgramCache;
use super::legacy_projection_retirement::{
    ThemeLegacyProjectionDisposition, ThemeLegacyProjectionProbeError,
    ThemeLegacyProjectionProbeReceipt, ThemeLegacyRouteValue,
};
use super::legacy_tombstones::{ThemeLegacyRouteFacet, ThemeLegacyRouteSelector};
use super::semantic::{OrdinalSelector, ThemeVariant};
use super::{DiagramThemeSpec, ThemeRule, ThemeRuleSet, ThemeStylePatch};
use crate::DiagramFamilyId;

/// Final route snapshot after removal of the production compatibility bridge.
///
/// A zero count is meaningful only when both sides are zero: the matrix must have no reachable
/// legacy route and the dispatch table must have no family left to send to the bridge. Keeping
/// these facts together prevents a migration from updating one ledger while leaving the other
/// executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFamilyThemeBridgeInventory {
    matrix_route_count: usize,
    matrix_family_count: usize,
    dispatched_family_count: usize,
    matrix_only_family_count: usize,
    dispatch_only_family_count: usize,
    dispatch_error_count: usize,
    matrix_route_digest: [u8; 32],
    matrix_family_digest: [u8; 32],
    dispatched_family_digest: [u8; 32],
}

impl LegacyFamilyThemeBridgeInventory {
    pub const fn matrix_route_count(self) -> usize {
        self.matrix_route_count
    }

    pub const fn matrix_family_count(self) -> usize {
        self.matrix_family_count
    }

    pub const fn dispatched_family_count(self) -> usize {
        self.dispatched_family_count
    }

    /// Families with a matrix-declared legacy route but no executable bridge dispatch.
    pub const fn matrix_only_family_count(self) -> usize {
        self.matrix_only_family_count
    }

    /// Families still dispatched to the bridge despite having no matrix-declared legacy route.
    pub const fn dispatch_only_family_count(self) -> usize {
        self.dispatch_only_family_count
    }

    pub const fn dispatch_error_count(self) -> usize {
        self.dispatch_error_count
    }

    /// Returns the canonical digest of the complete matrix-declared legacy route set.
    ///
    /// The digest is order-independent because the inventory is canonically sorted before it is
    /// hashed. It is intended for release/retirement ledgers that need to bind an exact route set,
    /// rather than merely asserting that the set is non-empty.
    pub const fn matrix_route_digest(self) -> [u8; 32] {
        self.matrix_route_digest
    }

    /// Returns the canonical digest of families with at least one matrix-declared legacy route.
    pub const fn matrix_family_digest(self) -> [u8; 32] {
        self.matrix_family_digest
    }

    /// Returns the canonical digest of families with an executable bridge dispatch.
    pub const fn dispatched_family_digest(self) -> [u8; 32] {
        self.dispatched_family_digest
    }

    /// Returns whether the production route and dispatch inventories are both empty.
    ///
    /// This is deliberately narrower than bridge-deletion authorization. The compiler provider,
    /// cutover and retirement ledgers, support claims, and release gates live outside this module
    /// and must be reconciled by the acceptance layer before the bridge can be removed.
    pub fn route_dispatch_is_empty(self) -> bool {
        self.matrix_route_count == 0
            && self.matrix_family_count == 0
            && self.dispatched_family_count == 0
            && self.matrix_only_family_count == 0
            && self.dispatch_only_family_count == 0
            && self.dispatch_error_count == 0
            && self.matrix_route_digest != [0; 32]
            && self.matrix_family_digest != [0; 32]
            && self.dispatched_family_digest != [0; 32]
    }
}

/// Returns the current matrix inventory and the final empty compatibility-dispatch snapshot.
///
/// Dispatch is permanently empty because the compiler has no family-overlay provider. The
/// independent migration, support, and release ledgers still reconcile the live matrix with
/// the retained historical witness; this snapshot does not replace those contracts.
pub fn legacy_family_theme_bridge_inventory() -> LegacyFamilyThemeBridgeInventory {
    let matrix_routes = legacy_compatibility_route_inventory();
    let matrix_families = matrix_routes
        .iter()
        .map(|route| match route {
            LegacyCompatibilityRouteKey::BaseTypography { family, .. }
            | LegacyCompatibilityRouteKey::RuleFacet { family, .. }
            | LegacyCompatibilityRouteKey::OrdinalPalette { family, .. }
            | LegacyCompatibilityRouteKey::EffectBinding { family, .. } => *family,
        })
        .collect::<BTreeSet<_>>();
    let dispatched_families = BTreeSet::new();
    let dispatch_error_count = 0;
    let matrix_only_family_count = matrix_families.difference(&dispatched_families).count();
    let dispatch_only_family_count = dispatched_families.difference(&matrix_families).count();

    LegacyFamilyThemeBridgeInventory {
        matrix_route_count: matrix_routes.len(),
        matrix_family_count: matrix_families.len(),
        dispatched_family_count: dispatched_families.len(),
        matrix_only_family_count,
        dispatch_only_family_count,
        dispatch_error_count,
        matrix_route_digest: digest_legacy_routes(&matrix_routes),
        matrix_family_digest: digest_family_set("matrix", &matrix_families),
        dispatched_family_digest: digest_family_set("dispatch", &dispatched_families),
    }
}

fn digest_legacy_routes(routes: &[LegacyCompatibilityRouteKey]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.legacy-family-theme-bridge.routes.v1");
    let mut routes = routes.to_vec();
    routes.sort_unstable();
    routes.dedup();
    update_len(&mut hasher, routes.len());
    for route in &routes {
        update_route_key(&mut hasher, route);
    }
    hasher.finalize().into()
}

fn digest_family_set(label: &str, families: &BTreeSet<DiagramFamilyId>) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.legacy-family-theme-bridge.families.v1",
    );
    update_len_prefixed(&mut hasher, label.as_bytes());
    update_len(&mut hasher, families.len());
    for family in families {
        update_len_prefixed(&mut hasher, family.as_str().as_bytes());
    }
    hasher.finalize().into()
}

fn update_route_key(hasher: &mut Sha256, route: &LegacyCompatibilityRouteKey) {
    match route {
        LegacyCompatibilityRouteKey::BaseTypography { family, property } => {
            update_len_prefixed(hasher, b"base-typography");
            update_len_prefixed(hasher, family.as_str().as_bytes());
            update_len_prefixed(hasher, property.id().as_bytes());
        }
        LegacyCompatibilityRouteKey::RuleFacet {
            family,
            target,
            selector,
            facet,
        } => {
            update_len_prefixed(hasher, b"rule-facet");
            update_len_prefixed(hasher, family.as_str().as_bytes());
            update_len_prefixed(hasher, target.id().as_bytes());
            update_selector(hasher, *selector);
            update_rule_facet(hasher, *facet);
        }
        LegacyCompatibilityRouteKey::OrdinalPalette { family, target } => {
            update_len_prefixed(hasher, b"ordinal-palette");
            update_len_prefixed(hasher, family.as_str().as_bytes());
            update_len_prefixed(hasher, target.id().as_bytes());
        }
        LegacyCompatibilityRouteKey::EffectBinding { family, target } => {
            update_len_prefixed(hasher, b"effect-binding");
            update_len_prefixed(hasher, family.as_str().as_bytes());
            update_len_prefixed(hasher, target.id().as_bytes());
        }
    }
}

fn update_selector(hasher: &mut Sha256, selector: FamilyThemeSelectorShape) {
    match selector {
        FamilyThemeSelectorShape::Static { variant } => {
            update_len_prefixed(hasher, b"static");
            update_optional_variant(hasher, variant);
        }
        FamilyThemeSelectorShape::Ordinal { variant, selector } => {
            update_len_prefixed(hasher, b"ordinal");
            update_optional_variant(hasher, variant);
            match selector {
                OrdinalSelector::Exact(index) => {
                    update_len_prefixed(hasher, b"exact");
                    update_len(hasher, index);
                }
                OrdinalSelector::Cycle { period, offset } => {
                    update_len_prefixed(hasher, b"cycle");
                    update_len(hasher, period);
                    update_len(hasher, offset);
                }
            }
        }
    }
}

fn update_optional_variant(hasher: &mut Sha256, variant: Option<ThemeVariant>) {
    match variant {
        Some(variant) => {
            hasher.update([1]);
            update_len_prefixed(hasher, variant.id().as_bytes());
        }
        None => hasher.update([0]),
    }
}

fn update_rule_facet(hasher: &mut Sha256, facet: FamilyThemeRuleFacet) {
    match facet {
        FamilyThemeRuleFacet::Fill(kind) => {
            update_len_prefixed(hasher, b"fill");
            update_paint_kind(hasher, kind);
        }
        FamilyThemeRuleFacet::Stroke(kind) => {
            update_len_prefixed(hasher, b"stroke");
            update_paint_kind(hasher, kind);
        }
        FamilyThemeRuleFacet::StrokeWidth => update_len_prefixed(hasher, b"stroke-width"),
        FamilyThemeRuleFacet::StrokeDasharray => update_len_prefixed(hasher, b"stroke-dasharray"),
        FamilyThemeRuleFacet::StrokeLinecap => update_len_prefixed(hasher, b"stroke-linecap"),
        FamilyThemeRuleFacet::StrokeLinejoin => update_len_prefixed(hasher, b"stroke-linejoin"),
        FamilyThemeRuleFacet::Opacity => update_len_prefixed(hasher, b"opacity"),
        FamilyThemeRuleFacet::FillOpacity => update_len_prefixed(hasher, b"fill-opacity"),
        FamilyThemeRuleFacet::StrokeOpacity => update_len_prefixed(hasher, b"stroke-opacity"),
        FamilyThemeRuleFacet::Radius => update_len_prefixed(hasher, b"radius"),
        FamilyThemeRuleFacet::Padding => update_len_prefixed(hasher, b"padding"),
        FamilyThemeRuleFacet::Typography(property) => {
            update_len_prefixed(hasher, b"typography");
            update_len_prefixed(hasher, property.id().as_bytes());
        }
        FamilyThemeRuleFacet::Effect => update_len_prefixed(hasher, b"effect"),
    }
}

fn update_paint_kind(hasher: &mut Sha256, kind: FamilyThemePaintKind) {
    let id = match kind {
        FamilyThemePaintKind::Clear => "clear",
        FamilyThemePaintKind::Transparent => "transparent",
        FamilyThemePaintKind::Solid => "solid",
        FamilyThemePaintKind::LinearGradient => "linear-gradient",
        FamilyThemePaintKind::RadialGradient => "radial-gradient",
        FamilyThemePaintKind::Pattern => "pattern",
    };
    update_len_prefixed(hasher, id.as_bytes());
}

fn update_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    update_len(hasher, bytes.len());
    hasher.update(bytes);
}

/// Observes the current matrix disposition and exact compatibility assignments for one route.
///
/// Historical before/after policy deliberately lives in the independent acceptance crate. This
/// function reports current production facts only, so adding a historical manifest row cannot
/// change the runtime matrix or suppress a compatibility-provider request.
pub(crate) fn legacy_projection_probe(
    id: super::legacy_tombstones::ThemeLegacyRouteId,
    value: ThemeLegacyRouteValue,
) -> Result<ThemeLegacyProjectionProbeReceipt, ThemeLegacyProjectionProbeError> {
    let selector = FamilyThemeSelectorShape::Static {
        variant: id.selector().variant(),
    };
    let paint_kind = match value {
        ThemeLegacyRouteValue::Transparent => FamilyThemePaintKind::Transparent,
        ThemeLegacyRouteValue::Solid => FamilyThemePaintKind::Solid,
    };
    let facet = match id.facet() {
        ThemeLegacyRouteFacet::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
        ThemeLegacyRouteFacet::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
    };

    let disposition = classify_rule_facet(id.family_id(), id.target(), selector, facet);
    let observed_disposition = match disposition {
        FamilyThemeDisposition::TypedAdapter => ThemeLegacyProjectionDisposition::TypedAdapter,
        FamilyThemeDisposition::LegacyCompatibility => {
            ThemeLegacyProjectionDisposition::LegacyCompatibility
        }
        FamilyThemeDisposition::Unsupported => ThemeLegacyProjectionDisposition::Unsupported,
    };

    let paint = match value {
        ThemeLegacyRouteValue::Transparent => CanvasPaint::Transparent,
        ThemeLegacyRouteValue::Solid => CanvasPaint::solid("#123456").map_err(|error| {
            ThemeLegacyProjectionProbeError::new(
                id,
                value,
                format!("solid probe construction failed: {error}"),
            )
        })?,
    };
    let style = match id.facet() {
        ThemeLegacyRouteFacet::Fill => ThemeStylePatch::default().with_fill(paint),
        ThemeLegacyRouteFacet::Stroke => ThemeStylePatch::default().with_stroke(paint),
    };
    let mut rule = ThemeRule::new(id.target(), style).for_family(id.family_id());
    if let ThemeLegacyRouteSelector::StaticVariant(variant) = id.selector() {
        rule = rule.with_variant(variant);
    }
    let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
    spec.validate().map_err(|error| {
        ThemeLegacyProjectionProbeError::new(
            id,
            value,
            format!("synthetic route is invalid: {error}"),
        )
    })?;

    let family_programs = FamilyThemeProgramCache::new(Arc::new(spec));
    // The production compiler no longer installs a family-overlay provider. Observe the
    // actual compiled program and fail closed if a route asks for the retired provider.
    let program = family_programs.get_or_compile(id.family_id());
    if program.has_legacy_compatibility() {
        return Err(ThemeLegacyProjectionProbeError::new(
            id,
            value,
            "family matrix declares a legacy route after compatibility bridge retirement",
        ));
    }
    ThemeLegacyProjectionProbeReceipt::seal(id, value, observed_disposition, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::super::family_mechanism_matrix::FamilyThemeMechanism;
    use super::super::resolved::ThemeTypographyProperty;
    use super::super::semantic::ThemeTarget;
    use super::super::typography::TextStyle;
    use super::*;
    use crate::diagram_theme::{
        FontStack, LineHeight, TextAlign, TextDecoration, TextTransform, ThemeWrapMode,
        TypographySpec, WhiteSpace,
    };

    #[test]
    fn bridge_inventory_is_derived_from_matrix_and_dispatch() {
        let status = legacy_family_theme_bridge_inventory();
        assert_eq!(status.dispatch_error_count(), 0);
        assert_eq!(status.matrix_route_count(), 0);
        assert_eq!(status.matrix_family_count(), 0);
        assert_eq!(status.dispatched_family_count(), 0);
        assert_eq!(status.matrix_only_family_count(), 0);
        assert_eq!(status.dispatch_only_family_count(), 0);
        assert!(status.route_dispatch_is_empty());
    }

    #[test]
    fn final_snapshot_rejects_route_reintroduction() {
        let mut status = legacy_family_theme_bridge_inventory();
        status.matrix_route_count = 1;
        status.matrix_family_count = 1;
        status.matrix_only_family_count = 1;
        assert!(!status.route_dispatch_is_empty());
    }

    #[test]
    fn bridge_deletion_gate_rejects_missing_route_digests() {
        let status = LegacyFamilyThemeBridgeInventory {
            matrix_route_count: 0,
            matrix_family_count: 0,
            dispatched_family_count: 0,
            matrix_only_family_count: 0,
            dispatch_only_family_count: 0,
            dispatch_error_count: 0,
            matrix_route_digest: [0; 32],
            matrix_family_digest: [1; 32],
            dispatched_family_digest: [1; 32],
        };

        assert!(!status.route_dispatch_is_empty());
    }

    #[test]
    fn legacy_projection_probe_reports_current_facts_without_retirement_policy() {
        let unsupported = legacy_projection_probe(
            super::super::legacy_tombstones::ThemeLegacyRouteId::new(
                DiagramFamilyId::CLASS,
                ThemeTarget::Marker,
                ThemeLegacyRouteSelector::StaticUnqualified,
                ThemeLegacyRouteFacet::Fill,
            ),
            ThemeLegacyRouteValue::Solid,
        )
        .expect("observe current Class marker facts");
        assert_eq!(
            unsupported.disposition(),
            ThemeLegacyProjectionDisposition::Unsupported
        );
        assert!(unsupported.projections().is_empty());

        let typed = legacy_projection_probe(
            super::super::legacy_tombstones::ThemeLegacyRouteId::new(
                DiagramFamilyId::C4,
                ThemeTarget::Text,
                ThemeLegacyRouteSelector::StaticUnqualified,
                ThemeLegacyRouteFacet::Fill,
            ),
            ThemeLegacyRouteValue::Solid,
        )
        .expect("observe current C4 text facts");
        assert_eq!(
            typed.disposition(),
            ThemeLegacyProjectionDisposition::TypedAdapter
        );
        assert!(typed.projections().is_empty());
    }

    #[test]
    fn base_typography_never_reenters_the_legacy_bridge() {
        for &family in DiagramFamilyId::all() {
            for &property in ThemeTypographyProperty::ALL {
                let typography = typography_probe_style(property);
                let spec = DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(family, typography),
                );
                let family_programs = FamilyThemeProgramCache::new(Arc::new(spec));
                let program = family_programs.get_or_compile(family);
                let route = program
                    .mechanism_routes()
                    .iter()
                    .find(|route| {
                        route.mechanism() == FamilyThemeMechanism::BaseTypography(property)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "typography probe did not compile family={family} property={}",
                            property.id()
                        )
                    });
                assert_ne!(
                    route.disposition(),
                    FamilyThemeDisposition::LegacyCompatibility,
                    "retired typography projection must not return: family={family} property={}",
                    property.id(),
                );
            }
        }
    }

    #[test]
    fn legacy_route_digest_detects_nonempty_routes_and_ignores_order() {
        // Use independent nonempty rows: reversing the now-empty live inventory proves nothing.
        let routes = vec![
            LegacyCompatibilityRouteKey::RuleFacet {
                family: DiagramFamilyId::CLASS,
                target: ThemeTarget::Node,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                facet: FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid),
            },
            LegacyCompatibilityRouteKey::RuleFacet {
                family: DiagramFamilyId::SEQUENCE,
                target: ThemeTarget::Actor,
                selector: FamilyThemeSelectorShape::Static { variant: None },
                facet: FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent),
            },
        ];
        let mut reversed = routes.clone();
        reversed.reverse();
        assert_eq!(
            digest_legacy_routes(&routes),
            digest_legacy_routes(&reversed)
        );
        assert_ne!(digest_legacy_routes(&routes), digest_legacy_routes(&[]));
    }

    #[test]
    fn final_retirement_probes_cover_every_historical_route_and_value() {
        let ids = super::super::legacy_tombstones::ktd23_tombstone_inventory()
            .expect("valid independent renderer-owned route identities");
        assert!(!ids.is_empty());
        for id in ids {
            for value in ThemeLegacyRouteValue::ALL {
                let probe = legacy_projection_probe(id, value)
                    .expect("retired route must compile without a compatibility provider");
                assert_eq!(probe.id(), id);
                assert_eq!(probe.value(), value);
                assert_ne!(
                    probe.disposition(),
                    ThemeLegacyProjectionDisposition::LegacyCompatibility
                );
                assert!(probe.projections().is_empty());
                assert_ne!(probe.digest(), [0; 32]);
            }
        }
    }

    fn typography_probe_style(property: ThemeTypographyProperty) -> TextStyle {
        match property {
            ThemeTypographyProperty::FontStack => TextStyle::default().with_font_stack(
                FontStack::new(["Legacy Probe", "sans-serif"]).expect("valid probe font stack"),
            ),
            ThemeTypographyProperty::FontSize => TextStyle::default()
                .with_font_size_px(18.0)
                .expect("valid probe font size"),
            ThemeTypographyProperty::FontWeight => TextStyle::default()
                .with_font_weight(700)
                .expect("valid probe font weight"),
            ThemeTypographyProperty::FontStyle => {
                TextStyle::default().with_font_style(crate::diagram_theme::FontStyle::Italic)
            }
            ThemeTypographyProperty::LineHeight => TextStyle::default()
                .with_line_height(LineHeight::Multiplier(1.5))
                .expect("valid probe line height"),
            ThemeTypographyProperty::LetterSpacing => TextStyle::default()
                .with_letter_spacing_px(1.0)
                .expect("valid probe letter spacing"),
            ThemeTypographyProperty::WordSpacing => TextStyle::default()
                .with_word_spacing_px(2.0)
                .expect("valid probe word spacing"),
            ThemeTypographyProperty::Transform => {
                TextStyle::default().with_transform(TextTransform::Uppercase)
            }
            ThemeTypographyProperty::Decoration => {
                TextStyle::default().with_decoration(TextDecoration::Underline)
            }
            ThemeTypographyProperty::TextAlign => {
                TextStyle::default().with_text_align(TextAlign::Center)
            }
            ThemeTypographyProperty::WhiteSpace => {
                TextStyle::default().with_white_space(WhiteSpace::PreWrap)
            }
            ThemeTypographyProperty::Wrap => {
                TextStyle::default().with_wrap(ThemeWrapMode::BreakWord)
            }
        }
    }
}
