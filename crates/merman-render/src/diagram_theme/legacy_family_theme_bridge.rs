use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

#[cfg(any(test, merman_internal_theme_acceptance))]
use std::collections::BTreeSet;

#[cfg(test)]
use merman_core::__private::ThemeFamilyCompatibilityOverlayBuilder;
use merman_core::__private::{ThemeCompatibilityOverlayError, ThemeFamilyCompatibilityOverlay};
use merman_core::{MermaidConfig, OperationControl, OperationControlResult};
#[cfg(test)]
use serde_json::Map;
use serde_json::Value;

#[cfg(any(test, merman_internal_theme_acceptance))]
use super::canvas::CanvasPaint;
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::family_mechanism_matrix::{FamilyThemeDisposition, FamilyThemeRuleFacet};
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::family_mechanism_matrix::{
    FamilyThemePaintKind, FamilyThemeSelectorShape, LegacyCompatibilityRouteKey,
    classify_rule_facet, legacy_compatibility_route_inventory,
};
use super::family_program::FamilyThemeProgramCache;
#[cfg(test)]
use super::semantic::ThemeTarget;
#[cfg(any(test, merman_internal_theme_acceptance))]
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::semantic::ThemeVariant;
use crate::DiagramFamilyId;

#[cfg(any(test, merman_internal_theme_acceptance))]
use super::semantic::OrdinalSelector;

#[cfg(any(test, merman_internal_theme_acceptance))]
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use super::family_mechanism_matrix::FamilyThemeMechanism;
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::legacy_projection_retirement::{
    ThemeLegacyProjectionDisposition, ThemeLegacyProjectionObservation,
    ThemeLegacyProjectionProbeError, ThemeLegacyProjectionProbeReceipt, ThemeLegacyRouteValue,
    value_digest,
};
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::legacy_tombstones::{ThemeLegacyRouteFacet, ThemeLegacyRouteSelector};
#[cfg(any(test, merman_internal_theme_acceptance))]
use super::{DiagramThemeSpec, ThemeRule, ThemeRuleSet, ThemeStylePatch};
#[cfg(test)]
use crate::theme_route_cutover::ThemeRouteCutoverProjection;

pub(super) const CONTRIBUTION_ID_PREFIX: &str = "merman.legacy-family-theme.v1.";

type BridgeResult<T> = Result<T, ThemeCompatibilityOverlayError>;
/// Temporary, family-local compatibility inputs for Mermaid renderers that do not yet consume the
/// typed theme program directly.
///
/// This bridge is deliberately lossy. Its provenance must be reported as legacy compatibility,
/// never as evidence that a typed mechanism was applied.
#[derive(Debug, Clone)]
pub(super) struct LegacyFamilyThemeBridge {
    inner: Arc<LegacyFamilyThemeBridgeInner>,
}

#[derive(Debug)]
struct LegacyFamilyThemeBridgeInner {
    family_programs: Arc<FamilyThemeProgramCache>,
    artifacts: BTreeMap<
        DiagramFamilyId,
        OnceLock<Result<LegacyFamilyThemeArtifact, ThemeCompatibilityOverlayError>>,
    >,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone))]
struct LegacyFamilyThemeArtifact {
    overlay: ThemeFamilyCompatibilityOverlay,
    #[cfg(test)]
    contribution_ids: BTreeSet<String>,
    #[cfg(any(test, merman_internal_theme_acceptance))]
    accepted_projections: BTreeSet<ThemeLegacyProjectionObservation>,
}

impl LegacyFamilyThemeBridge {
    pub(super) fn new(family_programs: Arc<FamilyThemeProgramCache>) -> Self {
        let artifacts = DiagramFamilyId::all()
            .iter()
            .copied()
            .map(|family| (family, OnceLock::new()))
            .collect();
        Self {
            inner: Arc::new(LegacyFamilyThemeBridgeInner {
                family_programs,
                artifacts,
            }),
        }
    }

    fn artifact_for_family(
        &self,
        family: DiagramFamilyId,
    ) -> Result<&LegacyFamilyThemeArtifact, ThemeCompatibilityOverlayError> {
        let artifact = self
            .inner
            .artifacts
            .get(&family)
            .ok_or_else(|| {
                ThemeCompatibilityOverlayError::provider_failure(
                    family.as_str(),
                    "family is absent from the built-in compatibility cache",
                )
            })?
            .get_or_init(|| compile_selected_family(&self.inner.family_programs, family));
        artifact.as_ref().map_err(Clone::clone)
    }

    #[cfg(test)]
    fn compile_for_family(&self, family: DiagramFamilyId) -> LegacyFamilyThemeArtifact {
        self.artifact_for_family(family)
            .expect("test family compatibility bridge must compile")
            .clone()
    }

    #[cfg(test)]
    fn cached_family_count(&self) -> usize {
        self.inner
            .artifacts
            .values()
            .filter(|artifact| artifact.get().is_some())
            .count()
    }

    #[cfg(test)]
    pub(super) fn owns_contribution_id(&self, opaque_id: &str) -> bool {
        let Some(family) = contribution_family(opaque_id) else {
            return false;
        };
        self.artifact_for_family(family)
            .expect("test family compatibility bridge must compile")
            .contribution_ids
            .contains(opaque_id)
    }

    pub(super) fn overlay_for_family(
        &self,
        family: &str,
        control: &OperationControl,
    ) -> OperationControlResult<
        Result<Option<ThemeFamilyCompatibilityOverlay>, ThemeCompatibilityOverlayError>,
    > {
        control.checkpoint()?;
        let Some(family) = DiagramFamilyId::from_id(family) else {
            return Ok(Err(ThemeCompatibilityOverlayError::provider_failure(
                family,
                "selected family is absent from the built-in legacy bridge catalog",
            )));
        };
        let artifact = self.artifact_for_family(family);
        control.checkpoint()?;
        Ok(artifact
            .map(|artifact| (!artifact.overlay.is_empty()).then(|| artifact.overlay.clone())))
    }
}

#[cfg(test)]
fn contribution_family(opaque_id: &str) -> Option<DiagramFamilyId> {
    let family = opaque_id
        .strip_prefix(CONTRIBUTION_ID_PREFIX)?
        .split_once('.')?
        .0;
    DiagramFamilyId::from_id(family)
}

fn compile_selected_family(
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) -> Result<LegacyFamilyThemeArtifact, ThemeCompatibilityOverlayError> {
    let program = family_programs.get_or_compile(family);
    if !program.has_legacy_compatibility() {
        return Ok(LegacyFamilyThemeArtifact::default());
    }

    Err(ThemeCompatibilityOverlayError::provider_failure(
        family.as_str(),
        "family matrix declares a legacy route after compatibility bridge retirement",
    ))
}

/// Matrix-derived readiness facts for retiring the compatibility bridge.
///
/// A zero count is meaningful only when both sides are zero: the matrix must have no reachable
/// legacy route and the dispatch table must have no family left to send to the bridge. Keeping
/// these facts together prevents a migration from updating one ledger while leaving the other
/// executable.
#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

/// Returns the production-owned route and dispatch inventory for the compatibility bridge.
///
/// This inventory proves only executable route ownership and dispatch consistency. It does not
/// authorize deleting the bridge; the acceptance layer must also reconcile provider removal and
/// the independent migration, support, and release ledgers.
#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
fn update_optional_variant(hasher: &mut Sha256, variant: Option<ThemeVariant>) {
    match variant {
        Some(variant) => {
            hasher.update([1]);
            update_len_prefixed(hasher, variant.id().as_bytes());
        }
        None => hasher.update([0]),
    }
}

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
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

#[cfg(any(test, merman_internal_theme_acceptance))]
fn update_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

#[cfg(any(test, merman_internal_theme_acceptance))]
fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    update_len(hasher, bytes.len());
    hasher.update(bytes);
}

/// Observes the current matrix disposition and exact compatibility assignments for one route.
///
/// Historical before/after policy deliberately lives in the independent acceptance crate. This
/// function reports current production facts only, so adding a historical manifest row cannot
/// change the runtime matrix or suppress a bridge projection.
#[cfg(any(test, merman_internal_theme_acceptance))]
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
    let artifact = compile_selected_family(&family_programs, id.family_id()).map_err(|error| {
        ThemeLegacyProjectionProbeError::new(
            id,
            value,
            format!("bridge projection failed: {error}"),
        )
    })?;
    if artifact.overlay.is_empty() != artifact.accepted_projections.is_empty() {
        return Err(ThemeLegacyProjectionProbeError::new(
            id,
            value,
            "bridge overlay and exact projection observations disagree",
        ));
    }
    ThemeLegacyProjectionProbeReceipt::seal(
        id,
        value,
        observed_disposition,
        artifact.accepted_projections.into_iter().collect(),
    )
}

#[cfg(test)]
fn solid_paint(paint: &CanvasPaint) -> Option<String> {
    match paint {
        CanvasPaint::Transparent => Some("transparent".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

#[cfg(test)]
struct FamilyContributions {
    entries: Vec<PendingContribution>,
}

#[cfg(test)]
struct PendingContribution {
    mapping: &'static str,
    patch: Map<String, Value>,
}

#[cfg(test)]
impl FamilyContributions {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add_theme_variables<const N: usize>(
        &mut self,
        mapping: &'static str,
        variables: [(&'static str, Option<String>); N],
    ) {
        let variables = variables
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key.to_string(), Value::String(value))))
            .collect::<Map<_, _>>();
        if variables.is_empty() {
            return;
        }
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch(mapping, root);
    }

    fn add_patch(&mut self, mapping: &'static str, patch: Map<String, Value>) {
        self.entries.push(PendingContribution { mapping, patch });
    }

    fn finish_into(self, builder: &mut OverlayBuilder) -> BridgeResult<()> {
        for contribution in self.entries {
            builder.push(contribution.mapping, contribution.patch)?;
        }
        Ok(())
    }
}

#[cfg(test)]
struct OverlayBuilder {
    overlay: ThemeFamilyCompatibilityOverlayBuilder,
    #[cfg(test)]
    contribution_ids: BTreeSet<String>,
    #[cfg(any(test, merman_internal_theme_acceptance))]
    accepted_projections: BTreeSet<ThemeLegacyProjectionObservation>,
}

#[cfg(test)]
impl OverlayBuilder {
    fn new(family: DiagramFamilyId) -> Self {
        Self {
            overlay: ThemeFamilyCompatibilityOverlayBuilder::new(
                family.as_str(),
                CONTRIBUTION_ID_PREFIX,
            ),
            #[cfg(test)]
            contribution_ids: BTreeSet::new(),
            #[cfg(any(test, merman_internal_theme_acceptance))]
            accepted_projections: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    fn push(&mut self, mapping: &'static str, patch: Map<String, Value>) -> BridgeResult<()> {
        let _receipt = self
            .overlay
            .try_push(mapping, MermaidConfig::from_value(Value::Object(patch)))?;

        #[cfg(test)]
        {
            let inserted = self
                .contribution_ids
                .insert(_receipt.opaque_id().to_string());
            debug_assert!(inserted, "core must reject duplicate contribution ids");
        }
        #[cfg(any(test, merman_internal_theme_acceptance))]
        {
            self.accepted_projections.extend(_receipt.assignments().map(
                |(assignment_path, value)| {
                    ThemeLegacyProjectionObservation::new(
                        _receipt.opaque_id().to_string(),
                        assignment_path.to_string(),
                        value_digest(value),
                    )
                },
            ));
        }
        Ok(())
    }

    fn finish(self) -> LegacyFamilyThemeArtifact {
        LegacyFamilyThemeArtifact {
            overlay: self.overlay.finish(),
            #[cfg(test)]
            contribution_ids: self.contribution_ids,
            #[cfg(any(test, merman_internal_theme_acceptance))]
            accepted_projections: self.accepted_projections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::resolved::ThemeTypographyProperty;
    use super::super::typography::TextStyle;
    use super::*;
    use crate::diagram_theme::{
        CanvasSpec, FontStack, LineHeight, MermaidThemeCompatibility, Specified, TextAlign,
        TextDecoration, TextTransform, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeWrapMode,
        TypographySpec, WhiteSpace,
    };
    use merman_core::__private::{
        ThemeCompatibilityPlan, install_theme_compatibility, theme_parse_evidence,
    };

    fn solid(value: &str) -> CanvasPaint {
        CanvasPaint::solid(value).expect("valid test color")
    }

    const GANTT_FIXTURE: &str =
        include_str!("../../../../fixtures/gantt/task_tags_combinations.mmd");
    const REQUIREMENT_FIXTURE: &str =
        include_str!("../../../../fixtures/requirement/relations.mmd");
    const PIE_FIXTURE: &str = include_str!(
        "../../../../fixtures/pie/upstream_cypress_pie_spec_should_render_a_pie_diagram_with_showdata_005.mmd"
    );
    const JOURNEY_FIXTURE: &str =
        include_str!("../../../../fixtures/journey/upstream_tasks_and_people.mmd");

    fn bridge(spec: &DiagramThemeSpec) -> LegacyFamilyThemeBridge {
        let spec = Arc::new(spec.clone());
        LegacyFamilyThemeBridge::new(Arc::new(FamilyThemeProgramCache::new(spec)))
    }

    fn parse(spec: &DiagramThemeSpec, source: &str) -> merman_core::ParseMetadata {
        parse_with_compatibility(spec, source, MermaidConfig::empty_object())
    }

    fn parse_with_compatibility(
        spec: &DiagramThemeSpec,
        source: &str,
        compatibility_config: MermaidConfig,
    ) -> merman_core::ParseMetadata {
        let bridge = bridge(spec);
        let resolver = bridge.clone();
        let plan = ThemeCompatibilityPlan::try_new(
            [0x5a; 32],
            compatibility_config,
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .expect("test compatibility plan");
        install_theme_compatibility(merman_core::Engine::new(), &plan)
            .parse_metadata_sync(source)
            .expect("test diagram should parse")
    }

    #[test]
    fn sequence_retired_text_and_title_projections_stay_empty() {
        for target in [ThemeTarget::Text, ThemeTarget::Title] {
            for selector in [
                ThemeLegacyRouteSelector::StaticUnqualified,
                ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Default),
            ] {
                for value in ThemeLegacyRouteValue::ALL {
                    let receipt = legacy_projection_probe(
                        super::super::legacy_tombstones::ThemeLegacyRouteId::new(
                            DiagramFamilyId::SEQUENCE,
                            target,
                            selector,
                            ThemeLegacyRouteFacet::Fill,
                        ),
                        value,
                    )
                    .unwrap();
                    assert_eq!(
                        receipt.disposition(),
                        ThemeLegacyProjectionDisposition::Unsupported
                    );
                    assert!(receipt.projections().is_empty());
                }
            }
        }
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
    fn direct_mindmap_palette_does_not_project_legacy_color_scale_values() {
        let palette =
            super::super::OrdinalPalette::new([
                super::super::ThemeColorValue::parse("#abcdef").expect("valid palette color")
            ])
            .expect("non-empty palette");
        let spec = DiagramThemeSpec::new()
            .with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_theme("dark")
                    .expect("valid Mermaid theme"),
            )
            .with_styles(ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette));
        let compatibility = spec.mermaid().to_mermaid_config();
        let baseline = parse_with_compatibility(
            &DiagramThemeSpec::new().with_mermaid_compatibility(spec.mermaid().clone()),
            "mindmap\nroot(Root)\n Child(Child)\n",
            compatibility.clone(),
        );
        let parsed =
            parse_with_compatibility(&spec, "mindmap\nroot(Root)\n Child(Child)\n", compatibility);

        assert_eq!(
            parsed.effective_config.get_str("themeVariables.cScale0"),
            baseline.effective_config.get_str("themeVariables.cScale0")
        );
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn direct_mindmap_and_gitgraph_edge_stroke_retire_only_the_edge_projection() {
        for family in [DiagramFamilyId::MINDMAP, DiagramFamilyId::GIT_GRAPH] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                    )
                    .for_family(family),
                ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert!(artifact.overlay.is_empty(), "family={family}");
            assert!(artifact.contribution_ids.is_empty(), "family={family}");
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
        }
    }

    #[test]
    fn direct_gitgraph_default_edge_stroke_does_not_create_a_compatibility_overlay() {
        let rule = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(solid("#22c55e")),
        )
        .with_variant(ThemeVariant::Default)
        .for_family(DiagramFamilyId::GIT_GRAPH);
        let bridge =
            bridge(&DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)));
        let artifact = bridge.compile_for_family(DiagramFamilyId::GIT_GRAPH);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn typed_treemap_text_and_title_fill_suppress_their_projections() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::TREEMAP);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.treemap.text.fill")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.treemap.title.fill")
        );
    }

    #[test]
    fn final_direct_slices_retire_their_real_legacy_paint_projections() {
        for family in [
            DiagramFamilyId::EVENT_MODELING,
            DiagramFamilyId::ISHIKAWA,
            DiagramFamilyId::VENN,
            DiagramFamilyId::ZENUML,
        ] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(solid("#334155")),
                        )
                        .for_family(family),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                        )
                        .for_family(family),
                    ),
            );
            let artifact = bridge(&spec).compile_for_family(family);
            let prefix = format!("merman.legacy-family-theme.v1.{}", family.as_str());

            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.text.fill")),
                "family={family}"
            );
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.title.fill")),
                "family={family}"
            );
        }
    }

    #[test]
    fn mindmap_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let mut styles = ThemeRuleSet::default();
        for target in [
            ThemeTarget::NodeLabel,
            ThemeTarget::Text,
            ThemeTarget::Title,
            ThemeTarget::EdgeLabelBackground,
            ThemeTarget::ClusterLabel,
        ] {
            styles = styles.with_rule(
                ThemeRule::new(target, ThemeStylePatch::default().with_fill(solid(color)))
                    .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        for target in [ThemeTarget::Marker, ThemeTarget::Cluster] {
            styles = styles.with_rule(
                ThemeRule::new(
                    target,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        styles = styles
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_stroke(solid(color)),
                )
                .with_variant(ThemeVariant::Default)
                .for_family(DiagramFamilyId::MINDMAP),
            );

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::MINDMAP);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn direct_mindmap_node_paint_does_not_mutate_parse_time_compatibility_config() {
        let source = "mindmap\nroot(Root)\n Child(Child)\n";
        let fill = "#123456";
        let stroke = "#654321";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid(fill))
                        .with_stroke(solid(stroke)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            ),
        );

        let baseline = parse(&DiagramThemeSpec::default(), source);
        let parsed = parse(&spec, source);

        for path in ["themeVariables.mainBkg", "themeVariables.nodeBorder"] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "path={path}"
            );
        }
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn tree_view_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let styles = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::ClusterLabel,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            );

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::TREE_VIEW);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn tree_view_no_legacy_dispatch_retires_explicit_marker_paint() {
        let source = "treeView-beta\nroot\n  child\n";
        let label = "#123456";
        let line = "#654321";
        let icon = "#abcdef";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid(label)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(solid(line)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default().with_stroke(solid(icon)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                ),
        );

        let parsed = parse(&spec, source);

        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.labelColor"),
            Some(label),
            "the retired label projection must not be recreated"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.lineColor"),
            Some(line),
            "the retired line projection must not be recreated"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.iconColor"),
            Some(icon),
            "typed Tree View Marker.stroke must not be projected into the compatibility config"
        );

        let evidence = theme_parse_evidence(&parsed);
        let contributions = evidence.fallback_contributions().collect::<Vec<_>>();
        assert_eq!(
            contributions
                .iter()
                .flat_map(|contribution| contribution.surviving_assignment_paths())
                .collect::<BTreeSet<_>>(),
            BTreeSet::new()
        );
        assert!(contributions.iter().all(|contribution| {
            contribution
                .surviving_assignment_value("themeVariables.treeView.iconColor")
                .is_none()
        }));
    }

    #[test]
    fn tree_view_no_legacy_dispatch_does_not_project_typed_default_paint() {
        let source = "treeView-beta\nroot\n  child\n";
        let label = "#123456";
        let line = "#654321";
        let text = "#abcdef";
        let edge_fill = "#fedcba";
        let icon = "#112233";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid(label)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW)
                    .with_variant(ThemeVariant::Default),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid(text)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW)
                    .with_variant(ThemeVariant::Default),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(solid(edge_fill)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW)
                    .with_variant(ThemeVariant::Default),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(solid(line)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW)
                    .with_variant(ThemeVariant::Default),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default().with_stroke(solid(icon)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                ),
        );

        let parsed = parse(&spec, source);

        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.labelColor"),
            Some(label),
            "typed Default NodeLabel.fill must not be projected"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.labelColor"),
            Some(text),
            "typed Default Text.fill must not be projected"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.lineColor"),
            Some(line),
            "typed Default Edge.stroke must not be projected"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.lineColor"),
            Some(edge_fill),
            "typed Default Edge.fill must not be projected"
        );
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.iconColor"),
            Some(icon),
            "typed Tree View Marker.stroke must not be projected"
        );
        let evidence = theme_parse_evidence(&parsed);
        let contributions = evidence.fallback_contributions().collect::<Vec<_>>();
        assert_eq!(
            contributions
                .iter()
                .flat_map(|contribution| contribution.surviving_assignment_paths())
                .collect::<BTreeSet<_>>(),
            BTreeSet::new()
        );
    }

    #[test]
    fn gitgraph_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let mut styles = ThemeRuleSet::default();
        for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
            styles = styles.with_rule(
                ThemeRule::new(target, ThemeStylePatch::default().with_fill(solid(color)))
                    .for_family(DiagramFamilyId::GIT_GRAPH),
            );
        }
        for target in [ThemeTarget::Marker, ThemeTarget::Cluster] {
            styles = styles.with_rule(
                ThemeRule::new(
                    target,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::GIT_GRAPH),
            );
        }

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::GIT_GRAPH);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn gitgraph_retired_node_paint_preserves_materialized_config() {
        const SOURCE: &str = "gitGraph\n  commit id: \"A\" tag: \"v1\"\n";
        const PATHS: [&str; 6] = [
            "primaryColor",
            "mainBkg",
            "tagLabelBackground",
            "primaryBorderColor",
            "nodeBorder",
            "tagLabelBorder",
        ];
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid("#123456"))
                        .with_stroke(solid("#abcdef")),
                )
                .for_family(DiagramFamilyId::GIT_GRAPH),
            ),
        );
        let resolver = bridge(&spec);
        let plan = ThemeCompatibilityPlan::try_new(
            [0x6c; 32],
            MermaidConfig::empty_object(),
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .unwrap();
        for theme_name in [
            "default",
            "base",
            "dark",
            "forest",
            "neutral",
            "neo",
            "neo-dark",
            "redux",
            "redux-dark",
            "redux-color",
            "redux-dark-color",
        ] {
            for explicit in std::iter::once(None).chain(PATHS.iter().copied().map(Some)) {
                let mut config = serde_json::json!({"theme": theme_name});
                if let Some(key) = explicit {
                    config["themeVariables"] = serde_json::json!({key: "#fedcba"});
                }
                let engine =
                    merman_core::Engine::new().with_site_config(MermaidConfig::from_value(config));
                let baseline = engine.parse_metadata_sync(SOURCE).unwrap();
                let parsed = install_theme_compatibility(engine, &plan)
                    .parse_metadata_sync(SOURCE)
                    .unwrap();
                assert_eq!(
                    parsed.effective_config.as_value(),
                    baseline.effective_config.as_value(),
                    "{theme_name}/{explicit:?}: retired node paint must not mutate materialized config"
                );
                for key in PATHS {
                    let path = format!("themeVariables.{key}");
                    assert_eq!(
                        merman_core::__private::fallback_overlay_owns_path(
                            &parsed.effective_config,
                            &path
                        ),
                        false,
                        "{theme_name}/{explicit:?}/{key}"
                    );
                }
                if theme_name == "base" && explicit == Some("primaryColor") {
                    assert!(merman_core::__private::config_path_overrides_typed_default(
                        &baseline.effective_config,
                        "themeVariables.primaryBorderColor"
                    ));
                    assert_eq!(
                        parsed
                            .effective_config
                            .get_str("themeVariables.primaryBorderColor"),
                        baseline
                            .effective_config
                            .get_str("themeVariables.primaryBorderColor"),
                        "derived config remains untouched after bridge retirement"
                    );
                }
            }
        }
    }

    #[test]
    fn gitgraph_source_selected_theme_does_not_turn_derived_colors_into_direct_owners() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid("#123456"))
                        .with_stroke(solid("#abcdef")),
                )
                .for_family(DiagramFamilyId::GIT_GRAPH),
            ),
        );
        let source = r##"---
config:
  theme: base
  themeVariables:
    primaryColor: '#fedcba'
---
gitGraph
  commit id: "A" tag: "v1"
"##;
        // Source theme variables require an explicit host policy opt-in.
        let engine = merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(serde_json::json!({"secure": []})));
        let baseline = engine.parse_metadata_sync(source).unwrap();
        assert_eq!(
            baseline
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#fedcba")
        );
        assert!(merman_core::__private::config_path_overrides_typed_default(
            &baseline.effective_config,
            "themeVariables.primaryBorderColor"
        ));
        let resolver = bridge(&spec);
        let plan = ThemeCompatibilityPlan::try_new(
            [0x6d; 32],
            MermaidConfig::empty_object(),
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .unwrap();
        let parsed = install_theme_compatibility(engine, &plan)
            .parse_metadata_sync(source)
            .unwrap();
        assert_eq!(
            parsed.effective_config.as_value(),
            baseline.effective_config.as_value(),
            "retired node paint preserves source theme reselection"
        );
    }

    #[test]
    fn gitgraph_direct_paint_leaves_no_legacy_projections() {
        let source = "gitGraph\n  commit id: \"A\" tag: \"v1\"\n";
        let node_fill = "#101112";
        let node_stroke = "#131415";
        let node_label = "#161718";
        let text = "#191a1b";
        let edge = "#1c1d1e";
        let edge_label = "#1f2021";
        let edge_label_background = "#222324";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(solid(node_fill))
                            .with_stroke(solid(node_stroke)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid(text)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid(node_label)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(solid(edge)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabel,
                        ThemeStylePatch::default().with_fill(solid(edge_label)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default().with_fill(solid(edge_label_background)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                ),
        );

        let parsed = parse(&spec, source);

        for (path, value) in [
            ("themeVariables.primaryBorderColor", node_stroke),
            ("themeVariables.nodeBorder", node_stroke),
            ("themeVariables.tagLabelBorder", node_stroke),
        ] {
            assert_ne!(
                parsed.effective_config.get_str(path),
                Some(value),
                "path={path}"
            );
        }
        assert_ne!(
            parsed
                .effective_config
                .get_str("themeVariables.commitLabelBackground"),
            Some(edge_label_background),
            "the retired projection must not be reintroduced through the bridge"
        );

        for (path, value) in [
            ("themeVariables.primaryColor", node_fill),
            ("themeVariables.mainBkg", node_fill),
            ("themeVariables.tagLabelBackground", node_fill),
            ("themeVariables.commitLineColor", edge),
            ("themeVariables.textColor", text),
            ("themeVariables.tagLabelColor", node_label),
            ("themeVariables.commitLabelColor", edge_label),
        ] {
            assert_ne!(
                parsed.effective_config.get_str(path),
                Some(value),
                "path={path}"
            );
        }
        let evidence = theme_parse_evidence(&parsed);
        let mut assignments = std::collections::BTreeMap::new();
        for contribution in evidence.fallback_contributions() {
            for path in contribution.surviving_assignment_paths() {
                let value = contribution
                    .surviving_assignment_value(path)
                    .and_then(Value::as_str)
                    .expect("paint compatibility assignments should be strings");
                assert_eq!(assignments.insert(path, value), None, "path={path}");
            }
        }
        assert!(assignments.is_empty());
        assert!(
            !assignments.contains_key("themeVariables.commitLabelBackground"),
            "the typed route must not leave a compatibility overlay assignment"
        );
    }

    #[test]
    fn gitgraph_direct_typography_and_node_fill_leave_no_fallback_contribution() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["GitGraph Direct", "sans-serif"])
                    .expect("valid GitGraph font stack"),
            )
            .with_font_size_px(23.0)
            .expect("valid GitGraph font size");
        let spec = DiagramThemeSpec::new()
            .with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::GIT_GRAPH, typography),
            )
            .with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(solid("#123456")),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                ),
            );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::GIT_GRAPH);

        let typography_id = "merman.legacy-family-theme.v1.gitGraph.typography";
        let node_fill_id = "merman.legacy-family-theme.v1.gitGraph.node.fill";
        assert!(!artifact.contribution_ids.contains(typography_id));
        assert!(!bridge.owns_contribution_id(typography_id));
        assert!(!artifact.contribution_ids.contains(node_fill_id));
        assert!(!bridge.owns_contribution_id(node_fill_id));

        let parsed = parse(&spec, "gitGraph\n  commit id: \"A\"\n");
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert!(
                !merman_core::__private::fallback_overlay_owns_path(&parsed.effective_config, path,),
                "GitGraph typography must not retain fallback ownership of `{path}`"
            );
        }
        assert_ne!(
            parsed.effective_config.get_str("themeVariables.mainBkg"),
            Some("#123456")
        );
    }

    #[test]
    fn gitgraph_edge_text_and_label_fill_retire_projections_and_preserve_explicit_config() {
        const SOURCE: &str = "gitGraph\n  commit id: \"A\" tag: \"v1\"\n";
        for target in [
            ThemeTarget::Node,
            ThemeTarget::Text,
            ThemeTarget::NodeLabel,
            ThemeTarget::EdgeLabel,
            ThemeTarget::Edge,
        ] {
            for paint in [CanvasPaint::Transparent, solid("#123456")] {
                for variant in [None, Some(ThemeVariant::Default)] {
                    let mut rule =
                        ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint.clone()))
                            .for_family(DiagramFamilyId::GIT_GRAPH);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let spec = DiagramThemeSpec::new()
                        .with_styles(ThemeRuleSet::default().with_rule(rule));
                    let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::GIT_GRAPH);
                    assert!(artifact.overlay.is_empty());
                    assert!(artifact.contribution_ids.is_empty());
                    let parsed = parse(&spec, SOURCE);
                    assert_eq!(fallback_contribution_count(&parsed), 0);

                    let compatibility = MermaidThemeCompatibility::default()
                        .with_variable("commitLineColor", "#112233")
                        .unwrap()
                        .with_variable("textColor", "#abcdef")
                        .unwrap()
                        .with_variable("tagLabelColor", "#445566")
                        .unwrap()
                        .with_variable("commitLabelColor", "#778899")
                        .unwrap();
                    let spec = spec.with_mermaid_compatibility(compatibility);
                    let parsed =
                        parse_with_compatibility(&spec, SOURCE, spec.mermaid().to_mermaid_config());
                    for (path, value) in [
                        ("commitLineColor", "#112233"),
                        ("textColor", "#abcdef"),
                        ("tagLabelColor", "#445566"),
                        ("commitLabelColor", "#778899"),
                    ] {
                        assert_eq!(
                            parsed
                                .effective_config
                                .get_str(&format!("themeVariables.{path}")),
                            Some(value),
                        );
                    }
                    assert_eq!(fallback_contribution_count(&parsed), 0);
                }
            }
        }
    }

    #[test]
    fn treemap_typed_font_stack_and_unsupported_font_size_are_not_projected_into_legacy_config() {
        const SOURCE: &str = "treemap\n\"Root\"\n  \"Leaf\": 1\n";

        let typography = TextStyle::default()
            .with_font_stack(
                FontStack::single("Treemap Legacy")
                    .expect("valid Treemap compatibility font stack"),
            )
            .with_font_size_px(24.0)
            .expect("valid Treemap font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::TREEMAP, typography),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::TREEMAP);
        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());

        let baseline = parse(&DiagramThemeSpec::default(), SOURCE);
        let themed = parse(&spec, SOURCE);
        assert_eq!(
            themed.effective_config.get_str("themeVariables.fontSize"),
            baseline.effective_config.get_str("themeVariables.fontSize"),
            "Treemap FontSize is Unsupported and must not enter the compatibility overlay"
        );
        assert_eq!(
            themed.effective_config.get_str("fontSize"),
            baseline.effective_config.get_str("fontSize"),
            "Treemap FontSize must not shadow the root fallback"
        );
        for path in ["fontFamily", "themeVariables.fontFamily"] {
            assert_eq!(
                themed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "Treemap FontStack must not enter the compatibility overlay"
            );
        }
    }

    fn fallback_contribution_count(metadata: &merman_core::ParseMetadata) -> usize {
        theme_parse_evidence(metadata).fallback_contribution_count()
    }

    #[test]
    fn radar_paint_rules_leave_materialized_mermaid_config_unchanged() {
        const SOURCE: &str = "radar-beta\ntitle Direct Radar\naxis A,B,C\ncurve Current{1,2,3}\n";
        let styles = [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Axis]
            .into_iter()
            .fold(ThemeRuleSet::default(), |styles, target| {
                styles.with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(solid("#123456")),
                    )
                    .for_family(DiagramFamilyId::RADAR),
                )
            });
        let spec = DiagramThemeSpec::new().with_styles(styles);
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::RADAR);
        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        let resolver = bridge(&spec);
        let plan = ThemeCompatibilityPlan::try_new(
            [0x5a; 32],
            MermaidConfig::empty_object(),
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .unwrap();
        for theme_name in ["default", "base", "dark"] {
            let engine = merman_core::Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"theme": theme_name}),
            ));
            let baseline = engine.parse_metadata_sync(SOURCE).unwrap();
            let themed = install_theme_compatibility(engine, &plan)
                .parse_metadata_sync(SOURCE)
                .unwrap();
            assert_eq!(
                themed.effective_config.as_value(),
                baseline.effective_config.as_value(),
                "{theme_name}"
            );
            assert_eq!(fallback_contribution_count(&themed), 0);
        }
    }

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
    fn bridge_route_dispatch_inventory_can_be_empty() {
        let empty_routes = digest_legacy_routes(&[]);
        let empty_families = BTreeSet::new();
        let status = LegacyFamilyThemeBridgeInventory {
            matrix_route_count: 0,
            matrix_family_count: 0,
            dispatched_family_count: 0,
            matrix_only_family_count: 0,
            dispatch_only_family_count: 0,
            dispatch_error_count: 0,
            matrix_route_digest: empty_routes,
            matrix_family_digest: digest_family_set("matrix", &empty_families),
            dispatched_family_digest: digest_family_set("dispatch", &empty_families),
        };

        assert!(status.route_dispatch_is_empty());
    }

    #[test]
    fn legacy_route_digest_is_independent_of_inventory_order() {
        let routes = legacy_compatibility_route_inventory();
        let mut reversed = routes.clone();
        reversed.reverse();

        assert_eq!(
            digest_legacy_routes(&routes),
            digest_legacy_routes(&reversed)
        );
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
    fn wardley_generic_text_paint_does_not_recreate_a_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .for_family(DiagramFamilyId::WARDLEY),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::WARDLEY),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::WARDLEY);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn generic_title_paint_does_not_recreate_a_legacy_projection_without_a_title_consumer() {
        for family in [
            DiagramFamilyId::ARCHITECTURE,
            DiagramFamilyId::C4,
            DiagramFamilyId::CYNEFIN,
            DiagramFamilyId::KANBAN,
            DiagramFamilyId::SANKEY,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                let text_rule = ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(solid("#334155")),
                )
                .for_family(family);
                let title_rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                )
                .for_family(family);
                let text_rule = if let Some(variant) = variant {
                    text_rule.with_variant(variant)
                } else {
                    text_rule
                };
                let title_rule = if let Some(variant) = variant {
                    title_rule.with_variant(variant)
                } else {
                    title_rule
                };
                let spec = DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(text_rule)
                        .with_rule(title_rule),
                );
                let bridge = bridge(&spec);
                let artifact = bridge.compile_for_family(family);
                let text_id = format!("{CONTRIBUTION_ID_PREFIX}{}.text.fill", family.as_str());
                let title_id = format!("{CONTRIBUTION_ID_PREFIX}{}.title.fill", family.as_str());

                let expects_text_bridge = !matches!(
                    family,
                    DiagramFamilyId::ARCHITECTURE
                        | DiagramFamilyId::C4
                        | DiagramFamilyId::CYNEFIN
                        | DiagramFamilyId::KANBAN
                        | DiagramFamilyId::SANKEY
                );
                assert_eq!(
                    artifact.contribution_ids.contains(&text_id),
                    expects_text_bridge,
                    "{family} generic text bridge ownership drifted for variant={variant:?}"
                );
                assert!(
                    !artifact.contribution_ids.contains(&title_id),
                    "{family} must not recreate the terminal-less title projection for variant={variant:?}"
                );
            }
        }
    }

    #[test]
    fn railroad_title_fill_is_no_longer_projected_through_the_legacy_bridge() {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(solid("#f8fafc")),
            )
            .for_family(DiagramFamilyId::RAILROAD);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(DiagramFamilyId::RAILROAD);

            assert!(artifact.overlay.is_empty(), "variant={variant:?}");
            assert!(
                !artifact
                    .contribution_ids
                    .contains("merman.legacy-family-theme.v1.railroad.title.fill"),
                "variant={variant:?}"
            );
        }
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
    fn every_matrix_legacy_route_reaches_the_bridge_with_its_probe_value() {
        let mut matrix_legacy_families = BTreeSet::new();
        for &family in DiagramFamilyId::all() {
            for &target in ThemeTarget::ALL {
                if target == ThemeTarget::Canvas || !target.valid_for(family) {
                    continue;
                }
                for variant in
                    std::iter::once(None).chain(ThemeVariant::ALL.iter().copied().map(Some))
                {
                    let selector = FamilyThemeSelectorShape::Static { variant };
                    for (paint_kind, paint) in [
                        (FamilyThemePaintKind::Transparent, CanvasPaint::Transparent),
                        (FamilyThemePaintKind::Solid, solid("#123456")),
                    ] {
                        for facet in [
                            FamilyThemeRuleFacet::Fill(paint_kind),
                            FamilyThemeRuleFacet::Stroke(paint_kind),
                        ] {
                            if classify_rule_facet(family, target, selector, facet)
                                != FamilyThemeDisposition::LegacyCompatibility
                            {
                                continue;
                            }
                            let style = match facet {
                                FamilyThemeRuleFacet::Fill(_) => {
                                    ThemeStylePatch::default().with_fill(paint.clone())
                                }
                                FamilyThemeRuleFacet::Stroke(_) => {
                                    ThemeStylePatch::default().with_stroke(paint.clone())
                                }
                                _ => unreachable!("paint probe facet is fixed above"),
                            };
                            let mut rule = ThemeRule::new(target, style).for_family(family);
                            if let Some(variant) = variant {
                                rule = rule.with_variant(variant);
                            }
                            let spec = DiagramThemeSpec::new()
                                .with_styles(ThemeRuleSet::default().with_rule(rule));
                            matrix_legacy_families.insert(family);
                            assert_legacy_probe_projects(
                                family,
                                spec,
                                FamilyThemeMechanism::RuleFacet {
                                    rule_index: 0,
                                    target,
                                    selector,
                                    facet,
                                },
                                value_digest(&Value::String(
                                    solid_paint(&paint)
                                        .expect("legacy scalar probe paint must serialize"),
                                )),
                                &format!(
                                    "target={} variant={} facet={facet:?}",
                                    target.id(),
                                    variant.map_or("unqualified", ThemeVariant::id),
                                ),
                            );
                        }
                    }
                }
            }
        }

        assert!(matrix_legacy_families.is_empty());
    }

    fn assert_legacy_probe_projects(
        family: DiagramFamilyId,
        spec: DiagramThemeSpec,
        expected_mechanism: FamilyThemeMechanism,
        expected_value_digest: [u8; 32],
        route: &str,
    ) {
        spec.validate().expect("legacy route probe must be valid");
        let family_programs = FamilyThemeProgramCache::new(Arc::new(spec));
        let program = family_programs.get_or_compile(family);
        let actual_mechanisms = program
            .mechanism_routes()
            .iter()
            .filter(|route| route.disposition() == FamilyThemeDisposition::LegacyCompatibility)
            .map(|route| route.mechanism())
            .collect::<Vec<_>>();
        assert_eq!(
            actual_mechanisms,
            vec![expected_mechanism],
            "legacy probe did not isolate the expected route: family={family} route={route}"
        );
        let artifact = compile_selected_family(&family_programs, family)
            .expect("matrix-declared legacy route must compile through the bridge");
        assert!(
            !artifact.overlay.is_empty() && !artifact.accepted_projections.is_empty(),
            "matrix-declared legacy route was silently dropped: family={family} route={route}"
        );
        // Exact assignment paths remain owned by family-specific mapping tests. Repeating them
        // here would turn this matrix-derived liveness guard into a second bridge manifest.
        assert!(
            artifact
                .accepted_projections
                .iter()
                .all(|projection| projection.value_digest() == expected_value_digest),
            "legacy route did not preserve its probe value: family={family} route={route}"
        );
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

    #[test]
    fn unknown_selected_family_fails_without_populating_the_cache() {
        let spec = Arc::new(DiagramThemeSpec::new());
        let family_programs = Arc::new(FamilyThemeProgramCache::new(Arc::clone(&spec)));
        let bridge = LegacyFamilyThemeBridge::new(Arc::clone(&family_programs));

        let error = bridge
            .overlay_for_family("future-family", &OperationControl::new())
            .expect("active control")
            .expect_err("unknown selected family must fail closed");

        assert!(error.to_string().contains("future-family"));
        assert_eq!(bridge.cached_family_count(), 0);
        assert_eq!(family_programs.len(), 0);
    }

    #[test]
    fn provider_compiles_only_the_selected_family_and_reuses_its_artifact() {
        let spec = Arc::new(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default().with_stroke(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        );
        let family_programs = Arc::new(FamilyThemeProgramCache::new(Arc::clone(&spec)));
        let bridge = LegacyFamilyThemeBridge::new(Arc::clone(&family_programs));
        let control = OperationControl::new();

        assert_eq!(bridge.cached_family_count(), 0);
        assert_eq!(family_programs.len(), 0);

        let first = bridge
            .overlay_for_family("block", &control)
            .expect("active control")
            .expect("valid retired block bridge");
        assert!(first.is_none());
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);
        assert!(family_programs.contains(DiagramFamilyId::BLOCK));
        assert!(!family_programs.contains(DiagramFamilyId::SEQUENCE));

        let second = bridge
            .overlay_for_family("block", &control)
            .expect("active control")
            .expect("cached valid retired block bridge");
        assert!(second.is_none());
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);

        assert!(
            bridge
                .overlay_for_family("state", &control)
                .expect("active control")
                .expect("valid bridge-free State classification")
                .is_none()
        );
        assert_eq!(bridge.cached_family_count(), 2);
        assert_eq!(family_programs.len(), 2);

        bridge
            .overlay_for_family("sequence", &control)
            .expect("active control")
            .expect("valid Sequence compatibility bridge");
        assert_eq!(bridge.cached_family_count(), 3);
        assert_eq!(family_programs.len(), 3);
        assert!(family_programs.contains(DiagramFamilyId::SEQUENCE));
    }

    #[test]
    fn direct_family_palettes_retire_git_graph_and_kanban_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#ef4444").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#22c55e").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Node, palette.clone())
            .with_ordinal_palette(ThemeTarget::Task, palette);
        let spec = DiagramThemeSpec::new().with_styles(styles);
        let bridge = bridge(&spec);

        let git_graph = bridge.compile_for_family(DiagramFamilyId::GIT_GRAPH);
        assert!(
            !git_graph
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.gitGraph.node.palette")
        );

        let kanban = bridge.compile_for_family(DiagramFamilyId::KANBAN);
        for retired_id in [
            "merman.legacy-family-theme.v1.kanban.task.palette.color-scale",
            "merman.legacy-family-theme.v1.kanban.task.palette.git",
        ] {
            assert!(!kanban.contribution_ids.contains(retired_id));
        }
    }

    #[test]
    fn unsupported_kanban_title_fill_preserves_explicit_compatibility() {
        const SOURCE: &str = "kanban\n  todo[Todo]\n    task[Task]\n";
        for paint in [CanvasPaint::Transparent, solid("#9333ea")] {
            for variant in [None, Some(ThemeVariant::Default)] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(paint.clone()),
                )
                .for_family(DiagramFamilyId::KANBAN);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let spec =
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
                let bridge_instance = bridge(&spec);
                let artifact = bridge_instance.compile_for_family(DiagramFamilyId::KANBAN);
                assert!(artifact.overlay.is_empty());
                assert!(artifact.contribution_ids.is_empty());
                assert!(
                    !bridge_instance
                        .owns_contribution_id("merman.legacy-family-theme.v1.kanban.title.fill")
                );

                let compatibility = MermaidThemeCompatibility::default()
                    .with_variable("titleColor", "#445566")
                    .expect("valid explicit Kanban compatibility title color");
                let spec = spec.with_mermaid_compatibility(compatibility);
                let parsed =
                    parse_with_compatibility(&spec, SOURCE, spec.mermaid().to_mermaid_config());
                assert_eq!(
                    parsed.effective_config.get_str("themeVariables.titleColor"),
                    Some("#445566")
                );
                assert_eq!(fallback_contribution_count(&parsed), 0);
            }
        }
    }

    #[test]
    fn kanban_text_fill_retires_only_its_family_bridge_and_preserves_explicit_config() {
        const SOURCE: &str = "kanban\n  todo[Todo]\n    task[Task]\n";
        for paint in [CanvasPaint::Transparent, solid("#16a34a")] {
            for variant in [None, Some(ThemeVariant::Default)] {
                let mut rule = ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(paint.clone()),
                );
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let spec =
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
                let bridge = bridge(&spec);
                let artifact = bridge.compile_for_family(DiagramFamilyId::KANBAN);
                assert!(artifact.overlay.is_empty());
                assert!(artifact.contribution_ids.is_empty());
                assert!(
                    !bridge.owns_contribution_id("merman.legacy-family-theme.v1.kanban.text.fill")
                );
                let chart = bridge.compile_for_family(DiagramFamilyId::XY_CHART);
                assert!(chart.contribution_ids.is_empty());
                assert!(chart.overlay.is_empty());

                let baseline = parse(&DiagramThemeSpec::new(), SOURCE);
                let parsed = parse(&spec, SOURCE);
                for path in ["themeVariables.textColor", "themeVariables.taskTextColor"] {
                    assert_eq!(
                        parsed.effective_config.get_str(path),
                        baseline.effective_config.get_str(path)
                    );
                }
                assert_eq!(fallback_contribution_count(&parsed), 0);
                let compatibility = MermaidThemeCompatibility::default()
                    .with_variable("textColor", "#445566")
                    .expect("valid explicit text color")
                    .with_variable("taskTextColor", "#112233")
                    .expect("valid explicit task text color");
                let spec = spec.with_mermaid_compatibility(compatibility);
                let parsed =
                    parse_with_compatibility(&spec, SOURCE, spec.mermaid().to_mermaid_config());
                assert_eq!(
                    parsed.effective_config.get_str("themeVariables.textColor"),
                    Some("#445566")
                );
                assert_eq!(
                    parsed
                        .effective_config
                        .get_str("themeVariables.taskTextColor"),
                    Some("#112233")
                );
                assert_eq!(fallback_contribution_count(&parsed), 0);
            }
        }
    }

    #[test]
    fn typed_kanban_task_stroke_has_no_bridge_contribution() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    ThemeStylePatch::default().with_stroke(solid("#334455")),
                )
                .for_family(DiagramFamilyId::KANBAN),
            ),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::KANBAN);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(
            !bridge(&spec).owns_contribution_id(
                ThemeRouteCutoverProjection::KanbanTaskStroke.contribution_id()
            )
        );
    }

    #[test]
    fn journey_direct_paint_and_unsupported_title_leave_the_legacy_bridge_empty() {
        let typography = TextStyle::default()
            .with_font_stack(FontStack::single("Journey Typed").expect("valid Journey font"))
            .with_font_size_px(21.0)
            .expect("valid Journey font size");
        let spec = DiagramThemeSpec::new()
            .with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::JOURNEY, typography),
            )
            .with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::JourneyTask,
                            ThemeStylePatch::default()
                                .with_fill(solid("#ef4444"))
                                .with_stroke(solid("#2563eb")),
                        )
                        .for_family(DiagramFamilyId::JOURNEY),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(solid("#16a34a")),
                        )
                        .for_family(DiagramFamilyId::JOURNEY),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(solid("#9333ea")),
                        )
                        .for_family(DiagramFamilyId::JOURNEY),
                    ),
            );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::JOURNEY);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.journey.title.fill"));

        let baseline = parse(&DiagramThemeSpec::new(), JOURNEY_FIXTURE);
        let parsed = parse(&spec, JOURNEY_FIXTURE);
        for path in ["themeVariables.textColor", "themeVariables.titleColor"] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "Journey must not project typed paint into {path}"
            );
        }
        assert_eq!(fallback_contribution_count(&parsed), 0);

        let compatibility = MermaidThemeCompatibility::default()
            .with_variable("titleColor", "#445566")
            .expect("valid explicit Journey compatibility title color");
        let spec = spec.with_mermaid_compatibility(compatibility);
        let parsed =
            parse_with_compatibility(&spec, JOURNEY_FIXTURE, spec.mermaid().to_mermaid_config());
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.titleColor"),
            Some("#445566")
        );
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn explicit_pie_slice_clear_does_not_create_a_direct_palette() {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
            ThemeRule::new(ThemeTarget::PieSlice, clear).for_family(DiagramFamilyId::PIE),
        ));
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PIE);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.palette")
        );
    }

    #[test]
    fn direct_pie_slice_fill_suppresses_both_fill_and_palette_bridge_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#ef4444").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#22c55e").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_fill(solid("#111827")),
                    )
                    .for_family(DiagramFamilyId::PIE),
                )
                .with_ordinal_palette(ThemeTarget::PieSlice, palette),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::PIE);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.fill")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.palette")
        );
    }

    #[test]
    fn direct_default_pie_slice_fill_has_no_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::PieSlice,
                    ThemeStylePatch::default().with_fill(solid("#111827")),
                )
                .for_family(DiagramFamilyId::PIE)
                .with_variant(ThemeVariant::Default),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PIE);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.pie.slice.fill"));
    }

    #[test]
    fn direct_default_pie_slice_stroke_has_no_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::PieSlice,
                    ThemeStylePatch::default().with_stroke(solid("#111827")),
                )
                .for_family(DiagramFamilyId::PIE)
                .with_variant(ThemeVariant::Default),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PIE);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.pie.slice.stroke"));
    }

    #[test]
    fn direct_pie_title_and_text_fill_suppress_their_bridge_contributions() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(solid("#111827")),
                    )
                    .for_family(DiagramFamilyId::PIE),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#b45309")),
                    )
                    .for_family(DiagramFamilyId::PIE),
                ),
        );
        let bridge_instance = bridge(&spec);
        let artifact = bridge_instance.compile_for_family(DiagramFamilyId::PIE);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.title.fill")
        );
        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(
            !bridge_instance.owns_contribution_id("merman.legacy-family-theme.v1.pie.title.fill")
        );
        assert!(
            !bridge_instance.owns_contribution_id("merman.legacy-family-theme.v1.pie.text.fill")
        );
    }

    #[test]
    fn family_scoped_rules_only_contribute_to_their_detected_family() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(solid("#ef4444")),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_fill(solid("#22c55e")),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
        );

        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_ne!(
            flowchart
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#ef4444")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
        assert_ne!(
            sequence.effective_config.get_str("themeVariables.actorBkg"),
            Some("#22c55e")
        );
        assert_eq!(fallback_contribution_count(&sequence), 0);
    }

    #[test]
    fn flowchart_roles_do_not_fall_back_into_sequence_roles() {
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )));

        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_eq!(fallback_contribution_count(&sequence), 0);
        assert_ne!(
            sequence.effective_config.get_str("themeVariables.actorBkg"),
            Some("#ef4444")
        );
    }

    #[test]
    fn swimlane_uses_its_own_program_and_stable_contribution_ids() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid("#fef3c7"))
                        .with_stroke(solid("#a16207")),
                )
                .for_family(DiagramFamilyId::SWIMLANE),
            ),
        );

        let swimlane = parse(&spec, "swimlane-beta LR\nA --> B\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        let baseline = parse(&DiagramThemeSpec::default(), "swimlane-beta LR\nA --> B\n");
        assert_eq!(fallback_contribution_count(&swimlane), 0);
        for path in [
            "themeVariables.clusterBkg",
            "themeVariables.secondaryColor",
            "themeVariables.clusterBorder",
        ] {
            assert_eq!(
                swimlane.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Swimlane Cluster paint must not mutate legacy `{path}`"
            );
        }
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn cluster_scalar_retires_all_legacy_assignments_without_rederiving_other_paint() {
        for (family, source) in [
            (
                DiagramFamilyId::CLASS,
                "classDiagram\nnamespace Group {\nclass A\nclass B\n}\nA --> B : relation\nnote for A \"memo\"\n",
            ),
            (
                DiagramFamilyId::BLOCK,
                "block-beta\n columns 1\n block:group[\"Group\"]\n A[\"Alpha\"]\n end\n B[\"Beta\"]\n",
            ),
        ] {
            for selected in ["base", "dark", "neo"] {
                for explicit_secondary in [None, Some("#456789")] {
                    let mut compatibility = MermaidThemeCompatibility::default()
                        .with_theme(selected)
                        .unwrap();
                    if let Some(color) = explicit_secondary {
                        compatibility = compatibility
                            .with_variable("secondaryColor", color)
                            .unwrap();
                    }
                    let baseline_spec =
                        DiagramThemeSpec::new().with_mermaid_compatibility(compatibility.clone());
                    let baseline = parse_with_compatibility(
                        &baseline_spec,
                        source,
                        baseline_spec.mermaid().to_mermaid_config(),
                    );
                    for paint in [CanvasPaint::Transparent, solid("#123456")] {
                        for variant in [None, Some(ThemeVariant::Default)] {
                            let mut rule = ThemeRule::new(
                                ThemeTarget::Cluster,
                                ThemeStylePatch::default()
                                    .with_fill(paint.clone())
                                    .with_stroke(paint.clone()),
                            );
                            if let Some(variant) = variant {
                                rule = rule.with_variant(variant);
                            }
                            let spec = baseline_spec
                                .clone()
                                .with_styles(ThemeRuleSet::default().with_rule(rule));
                            let artifact = bridge(&spec).compile_for_family(family);
                            assert!(artifact.overlay.is_empty());
                            assert!(artifact.contribution_ids.is_empty());
                            let parsed = parse_with_compatibility(
                                &spec,
                                source,
                                spec.mermaid().to_mermaid_config(),
                            );
                            assert_eq!(fallback_contribution_count(&parsed), 0);
                            for key in [
                                "clusterBkg",
                                "clusterBorder",
                                "secondaryColor",
                                "noteBkgColor",
                                "noteBorderColor",
                                "noteTextColor",
                                "edgeLabelBackground",
                                "secondaryBorderColor",
                                "gradientStop",
                            ] {
                                let path = format!("themeVariables.{key}");
                                assert_eq!(
                                    parsed.effective_config.get_str(&path),
                                    baseline.effective_config.get_str(&path),
                                    "{selected}/{key}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn family_typography_is_local_and_preserves_the_patch_shape() {
        let class_typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"]).expect("valid font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CLASS, class_typography),
        );

        let class = parse(&spec, "classDiagram\nclass Alpha\n");
        let class_baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&class), 0);
        assert_ne!(class.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            class.effective_config.get_str("fontFamily"),
            class_baseline.effective_config.get_str("fontFamily")
        );
        assert_eq!(
            class.effective_config.get_str("themeVariables.fontFamily"),
            class_baseline
                .effective_config
                .get_str("themeVariables.fontFamily")
        );
        assert_eq!(
            class.effective_config.get_str("themeVariables.fontSize"),
            class_baseline
                .effective_config
                .get_str("themeVariables.fontSize")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn direct_text_families_never_recreate_retired_typography_projections() {
        let font_stack = super::super::FontStack::new(["Inter", "sans-serif"])
            .expect("valid direct typography font stack");
        let font_size = TextStyle::default()
            .with_font_size_px(18.0)
            .expect("valid direct typography font size");

        for (family, source) in [
            (
                DiagramFamilyId::ARCHITECTURE,
                "architecture-beta\nservice api(server)[API]\n",
            ),
            (DiagramFamilyId::CLASS, "classDiagram\nclass Alpha\n"),
            (
                DiagramFamilyId::EVENT_MODELING,
                "eventmodeling\ntf 01 ui View\n",
            ),
            (DiagramFamilyId::ISHIKAWA, "ishikawa-beta\n Root cause\n"),
            (
                DiagramFamilyId::RADAR,
                "radar-beta\ntitle Direct radar\naxis A,B,C\ncurve Current{1,2,3}\n",
            ),
        ] {
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for typography in [
                TextStyle::default().with_font_stack(font_stack.clone()),
                font_size.clone(),
                font_size.clone().with_font_stack(font_stack.clone()),
            ] {
                let spec = DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(family, typography),
                );
                let artifact = bridge(&spec).compile_for_family(family);
                assert!(artifact.overlay.is_empty(), "{family} emitted an overlay");
                assert!(
                    artifact.contribution_ids.is_empty(),
                    "{family} emitted a contribution"
                );

                let parsed = parse(&spec, source);
                assert_eq!(fallback_contribution_count(&parsed), 0);
                for path in [
                    "fontFamily",
                    "themeVariables.fontFamily",
                    "themeVariables.fontSize",
                ] {
                    assert_eq!(
                        parsed.effective_config.get_str(path),
                        baseline.effective_config.get_str(path),
                        "direct {family} typography must not write fallback `{path}`"
                    );
                }
            }
        }
    }

    #[test]
    fn packet_font_stack_retires_the_legacy_typography_projection() {
        let typography = TextStyle::default().with_font_stack(
            super::super::FontStack::new(["Inter", "sans-serif"]).expect("valid Packet font stack"),
        );
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PACKET);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.packet.typography"));
    }

    #[test]
    fn property_local_font_stack_families_never_recreate_retired_typography_projections() {
        for (family, source) in [
            (
                DiagramFamilyId::VENN,
                "venn-beta\ntitle Venn\nset A[\"Alpha\"]:20\nset B[\"Beta\"]:12\n",
            ),
            (DiagramFamilyId::SANKEY, "sankey-beta\nA,B,10\n"),
            (DiagramFamilyId::PIE, "pie\n  \"Alpha\" : 1\n"),
        ] {
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for typography in [
                TextStyle::default().with_font_stack(
                    super::super::FontStack::new(["TypedSans", "sans-serif"])
                        .expect("valid direct font stack"),
                ),
                TextStyle::default()
                    .with_font_size_px(24.0)
                    .expect("valid unsupported font size"),
                TextStyle::default()
                    .with_font_stack(
                        super::super::FontStack::new(["MixedSans", "sans-serif"])
                            .expect("valid mixed font stack"),
                    )
                    .with_font_size_px(24.0)
                    .expect("valid mixed font size"),
            ] {
                let spec = DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(family, typography),
                );
                let artifact = bridge(&spec).compile_for_family(family);
                assert!(artifact.overlay.is_empty(), "family={family}");
                assert!(artifact.contribution_ids.is_empty(), "family={family}");

                let parsed = parse(&spec, source);
                assert_eq!(fallback_contribution_count(&parsed), 0, "family={family}");
                for path in [
                    "fontFamily",
                    "themeVariables.fontFamily",
                    "themeVariables.fontSize",
                ] {
                    assert_eq!(
                        parsed.effective_config.get_str(path),
                        baseline.effective_config.get_str(path),
                        "{family} typography must not mutate legacy `{path}`"
                    );
                }
            }
        }
    }

    #[test]
    fn requirement_base_typography_is_property_local() {
        let font_stack = super::super::FontStack::new(["Requirement Typed", "sans-serif"])
            .expect("valid Requirement font stack");
        let direct_spec =
            DiagramThemeSpec::new().with_typography(TypographySpec::default().with_family_style(
                DiagramFamilyId::REQUIREMENT,
                TextStyle::default().with_font_stack(font_stack.clone()),
            ));
        let direct_bridge = bridge(&direct_spec).compile_for_family(DiagramFamilyId::REQUIREMENT);
        assert!(direct_bridge.overlay.is_empty());
        assert!(direct_bridge.contribution_ids.is_empty());

        let mixed_typography = TextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .expect("valid mixed Requirement typography");
        let mixed_spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(DiagramFamilyId::REQUIREMENT, mixed_typography),
        );
        let mixed_bridge = bridge(&mixed_spec).compile_for_family(DiagramFamilyId::REQUIREMENT);
        assert!(mixed_bridge.contribution_ids.is_empty());

        let baseline = parse(&DiagramThemeSpec::default(), REQUIREMENT_FIXTURE);
        let mixed = parse(&mixed_spec, REQUIREMENT_FIXTURE);
        assert_eq!(fallback_contribution_count(&mixed), 0);
        for path in ["fontFamily", "themeVariables.fontFamily"] {
            assert_eq!(
                mixed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Requirement FontStack must not write legacy `{path}`"
            );
        }
        assert_eq!(
            mixed.effective_config.get_str("themeVariables.fontSize"),
            baseline.effective_config.get_str("themeVariables.fontSize")
        );
    }

    #[test]
    fn kanban_base_typography_is_property_local() {
        const SOURCE: &str = "kanban\n  todo[Todo]\n    task[Task]\n";

        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Kanban Typed", "sans-serif"])
                    .expect("valid Kanban font stack"),
            )
            .with_font_size_px(24.0)
            .expect("valid Kanban font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::KANBAN, typography),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::KANBAN);
        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());

        let baseline = parse(&DiagramThemeSpec::default(), SOURCE);
        let parsed = parse(&spec, SOURCE);
        assert_eq!(fallback_contribution_count(&parsed), 0);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Kanban typography must not write legacy `{path}`"
            );
        }
    }

    #[test]
    fn quadrant_chart_font_stack_is_property_local_while_font_size_is_unsupported() {
        const SOURCE: &str = "quadrantChart\nFeature: [0.5, 0.5]\n";

        let font_stack = super::super::FontStack::new(["Quadrant Typed", "sans-serif"])
            .expect("valid Quadrant Chart font stack");
        let direct_spec =
            DiagramThemeSpec::new().with_typography(TypographySpec::default().with_family_style(
                DiagramFamilyId::QUADRANT_CHART,
                TextStyle::default().with_font_stack(font_stack.clone()),
            ));
        let direct_bridge =
            bridge(&direct_spec).compile_for_family(DiagramFamilyId::QUADRANT_CHART);
        assert!(direct_bridge.overlay.is_empty());
        assert!(direct_bridge.contribution_ids.is_empty());

        let mixed_typography = TextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .expect("valid mixed Quadrant Chart typography");
        let mixed_spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(DiagramFamilyId::QUADRANT_CHART, mixed_typography),
        );
        let mixed_bridge = bridge(&mixed_spec).compile_for_family(DiagramFamilyId::QUADRANT_CHART);
        assert!(mixed_bridge.overlay.is_empty());
        assert!(mixed_bridge.contribution_ids.is_empty());

        let baseline = parse(&DiagramThemeSpec::default(), SOURCE);
        let mixed = parse(&mixed_spec, SOURCE);
        assert_eq!(fallback_contribution_count(&mixed), 0);
        assert_eq!(
            mixed.effective_config.get_str("themeVariables.fontSize"),
            baseline.effective_config.get_str("themeVariables.fontSize")
        );
        for path in ["fontFamily", "themeVariables.fontFamily"] {
            assert_eq!(
                mixed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Quadrant Chart FontStack must not write legacy `{path}`"
            );
        }
    }

    #[test]
    fn timeline_font_stack_and_font_size_are_property_local() {
        const SOURCE: &str = "timeline\n    title Typography\n    section Plan\n    Task : Event\n";

        let font_stack = super::super::FontStack::new(["Timeline Typed", "sans-serif"])
            .expect("valid Timeline font stack");
        let direct_spec =
            DiagramThemeSpec::new().with_typography(TypographySpec::default().with_family_style(
                DiagramFamilyId::TIMELINE,
                TextStyle::default().with_font_stack(font_stack.clone()),
            ));
        let direct_bridge = bridge(&direct_spec).compile_for_family(DiagramFamilyId::TIMELINE);
        assert!(direct_bridge.overlay.is_empty());
        assert!(direct_bridge.contribution_ids.is_empty());

        let mixed_typography = TextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .expect("valid mixed Timeline typography");
        let mixed_spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(DiagramFamilyId::TIMELINE, mixed_typography),
        );
        let mixed_bridge = bridge(&mixed_spec).compile_for_family(DiagramFamilyId::TIMELINE);
        assert!(
            !mixed_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.timeline.typography")
        );

        let baseline = parse(&DiagramThemeSpec::default(), SOURCE);
        let mixed = parse(&mixed_spec, SOURCE);
        assert_eq!(fallback_contribution_count(&mixed), 0);
        assert_eq!(
            mixed.effective_config.get_str("themeVariables.fontSize"),
            Some("16px")
        );
        for path in ["fontFamily", "themeVariables.fontFamily"] {
            assert_eq!(
                mixed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Timeline FontStack must not write legacy `{path}`"
            );
        }
    }

    #[test]
    fn railroad_typed_typography_and_text_fill_leave_no_legacy_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Railroad font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Railroad font size");
        let spec = DiagramThemeSpec::new()
            .with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::RAILROAD, typography),
            )
            .with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .for_family(DiagramFamilyId::RAILROAD),
                ),
            );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::RAILROAD);

        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.railroad.typography"));
        assert!(artifact.overlay.is_empty());
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.railroad.text.fill")
        );
        let source = include_str!(
            "../../../../fixtures/railroad/upstream_cypress_railroad_spec_renders_a_simple_rule_001.mmd"
        );
        let parsed = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Railroad typography must not write legacy `{path}`"
            );
        }
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.textColor"),
            baseline
                .effective_config
                .get_str("themeVariables.textColor")
        );
    }

    #[test]
    fn flowchart_and_swimlane_font_stack_and_size_retire_the_legacy_typography_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Flowchart-family font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Flowchart-family font size");

        for (family, source) in [
            (DiagramFamilyId::FLOWCHART, "flowchart LR\nA --> B\n"),
            (
                DiagramFamilyId::SWIMLANE,
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
            ),
        ] {
            let spec = DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(family, typography.clone()),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let contribution_id = format!(
                "merman.legacy-family-theme.v1.{}.typography",
                family.as_str()
            );

            assert!(!artifact.contribution_ids.contains(&contribution_id));
            assert!(!bridge.owns_contribution_id(&contribution_id));
            assert!(artifact.overlay.is_empty());

            let parsed = parse(&spec, source);
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for path in [
                "fontFamily",
                "themeVariables.fontFamily",
                "themeVariables.fontSize",
            ] {
                assert_eq!(
                    parsed.effective_config.get_str(path),
                    baseline.effective_config.get_str(path),
                    "typed {family} base typography must not write legacy `{path}`"
                );
            }
        }
    }

    #[test]
    fn sequence_font_stack_and_size_retire_the_legacy_typography_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Sequence font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Sequence font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::SEQUENCE, typography),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.typography"));

        let source = "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello\n";
        let parsed = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Sequence base typography must not write legacy `{path}`"
            );
        }
    }

    #[test]
    fn info_and_error_font_stack_suppression_preserves_only_real_terminal_paint_routes() {
        for family in [DiagramFamilyId::INFO, DiagramFamilyId::ERROR] {
            let typography = TextStyle::default()
                .with_font_stack(
                    super::super::FontStack::new(["Inter", "sans-serif"])
                        .expect("valid Info/Error font stack"),
                )
                .with_font_size_px(18.0)
                .expect("valid fixed-terminal residual font size");
            let spec = DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_family_style(family, typography))
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default().with_fill(solid("#334155")),
                            )
                            .for_family(family),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Title,
                                ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                            )
                            .for_family(family),
                        ),
                );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let prefix = format!("merman.legacy-family-theme.v1.{}", family.as_str());

            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.typography")),
                "{family} must not retain the migrated typography contribution"
            );
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.title.fill")),
                "{family} has no Title terminal and must not retain a Title bridge"
            );
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.text.fill")),
                "Info and Error text paint must not recreate a legacy contribution"
            );
            assert!(artifact.overlay.is_empty());
            assert!(artifact.contribution_ids.is_empty());
        }
    }

    #[test]
    fn cynefin_and_wardley_mixed_typography_never_recreates_the_legacy_projection() {
        for (family, source) in [
            (
                DiagramFamilyId::CYNEFIN,
                include_str!(
                    "../../../../fixtures/cynefin/upstream_cypress_cynefin_spec_should_render_a_simple_cynefin_diagram_with_all_five_domains_001.mmd"
                ),
            ),
            (
                DiagramFamilyId::WARDLEY,
                include_str!(
                    "../../../../fixtures/wardley/upstream_cypress_wardley_spec_1_should_render_tea_shop_001.mmd"
                ),
            ),
        ] {
            let typography = TextStyle::default()
                .with_font_stack(
                    super::super::FontStack::new(["Inter", "sans-serif"])
                        .expect("valid Cynefin/Wardley font stack"),
                )
                .with_font_size_px(18.0)
                .expect("valid unsupported Cynefin/Wardley font size");
            let spec = DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_family_style(family, typography));
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let contribution_id = format!(
                "merman.legacy-family-theme.v1.{}.typography",
                family.as_str()
            );

            assert!(!artifact.contribution_ids.contains(&contribution_id));
            assert!(!bridge.owns_contribution_id(&contribution_id));

            let parsed = parse(&spec, source);
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for path in [
                "fontFamily",
                "themeVariables.fontFamily",
                "themeVariables.fontSize",
            ] {
                assert_eq!(
                    parsed.effective_config.get_str(path),
                    baseline.effective_config.get_str(path),
                    "typed/unsupported {family} typography must not write legacy `{path}`"
                );
            }
        }
    }

    #[test]
    fn pie_and_block_static_stroke_routes_retire_their_direct_projection() {
        for (family, target, contribution_id) in [
            (
                DiagramFamilyId::PIE,
                ThemeTarget::PieSlice,
                "merman.legacy-family-theme.v1.pie.slice.stroke",
            ),
            (
                DiagramFamilyId::BLOCK,
                ThemeTarget::Node,
                "merman.legacy-family-theme.v1.block.node.stroke",
            ),
        ] {
            let direct_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                    )
                    .for_family(family),
                ),
            );
            let direct_bridge = bridge(&direct_spec);
            let direct = direct_bridge.compile_for_family(family);

            assert!(!direct.contribution_ids.contains(contribution_id));
            assert!(!direct_bridge.owns_contribution_id(contribution_id));

            let legacy_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(family),
                ),
            );
            let legacy_bridge = bridge(&legacy_spec);
            let legacy = legacy_bridge.compile_for_family(family);

            assert!(!legacy.contribution_ids.contains(contribution_id));
            assert!(!legacy_bridge.owns_contribution_id(contribution_id));
        }
    }

    #[test]
    fn block_background_has_no_legacy_projection_after_typed_cutover() {
        for variant in [None, Some(ThemeVariant::Default)] {
            for paint in [solid("#b316cd"), CanvasPaint::Transparent] {
                let rule = ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(paint),
                )
                .for_family(DiagramFamilyId::BLOCK);
                let rule = match variant {
                    Some(variant) => rule.with_variant(variant),
                    None => rule,
                };
                let spec =
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
                let bridge = bridge(&spec);
                let projection = bridge.compile_for_family(DiagramFamilyId::BLOCK);
                assert!(projection.overlay.is_empty());
                assert!(projection.contribution_ids.is_empty());
                assert!(!bridge.owns_contribution_id(
                    "merman.legacy-family-theme.v1.block.edge-label-background.fill"
                ));
            }
        }
    }

    #[test]
    fn block_static_fill_routes_retire_their_direct_projection() {
        let contribution_id = "merman.legacy-family-theme.v1.block.node.fill";
        let direct_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#654321")),
                )
                .for_family(DiagramFamilyId::BLOCK),
            ),
        );
        let direct_bridge = bridge(&direct_spec);
        let direct = direct_bridge.compile_for_family(DiagramFamilyId::BLOCK);

        assert!(direct.overlay.is_empty());
        assert!(!direct.contribution_ids.contains(contribution_id));
        assert!(!direct_bridge.owns_contribution_id(contribution_id));

        let legacy_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#654321")),
                )
                .with_variant(ThemeVariant::Default)
                .for_family(DiagramFamilyId::BLOCK),
            ),
        );
        let legacy_bridge = bridge(&legacy_spec);
        let legacy = legacy_bridge.compile_for_family(DiagramFamilyId::BLOCK);

        let parsed = parse(&legacy_spec, "block\n  A[\"Alpha\"]\n");
        let baseline = parse(&DiagramThemeSpec::default(), "block\n  A[\"Alpha\"]\n");
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.primaryColor"),
            baseline
                .effective_config
                .get_str("themeVariables.primaryColor")
        );
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.mainBkg"),
            baseline.effective_config.get_str("themeVariables.mainBkg")
        );
        assert!(!legacy.contribution_ids.contains(contribution_id));
        assert!(!legacy_bridge.owns_contribution_id(contribution_id));
    }

    #[test]
    fn unsupported_base_typography_does_not_emit_unrelated_legacy_fields() {
        let sequence_typography = TextStyle::default()
            .with_font_weight(700)
            .expect("valid font weight");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(DiagramFamilyId::SEQUENCE, sequence_typography),
        );
        let source = "sequenceDiagram\nAlice->>Bob: Hello\n";

        let sequence = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);

        assert_eq!(fallback_contribution_count(&sequence), 0);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                sequence.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "unsupported base typography must not mutate `{path}`"
            );
        }
    }

    #[test]
    fn oversized_class_typography_does_not_recreate_legacy_overlay() {
        let families = (0..32)
            .map(|index| format!("font-{index}-{}", "x".repeat(180)))
            .collect::<Vec<_>>();
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(families).expect("valid oversized font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CLASS, typography),
        );
        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");

        assert_eq!(fallback_contribution_count(&parsed), 0);
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.fontSize"),
            baseline.effective_config.get_str("themeVariables.fontSize")
        );
        assert_eq!(
            parsed.effective_config.get_str("fontFamily"),
            baseline.effective_config.get_str("fontFamily")
        );
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.fontFamily"),
            baseline
                .effective_config
                .get_str("themeVariables.fontFamily")
        );
    }

    #[test]
    fn state_and_root_owned_mechanisms_do_not_create_legacy_overlays() {
        let state_rule = ThemeRule::new(
            ThemeTarget::State,
            ThemeStylePatch::default().with_fill(solid("#2563eb")),
        )
        .for_family(DiagramFamilyId::STATE);
        let state_only = DiagramThemeSpec::new()
            .with_canvas(CanvasSpec::solid("#0f172a").expect("valid canvas"))
            .with_styles(ThemeRuleSet::default().with_rule(state_rule));
        let bridge = bridge(&state_only);

        assert_eq!(bridge.cached_family_count(), 0);
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill"));
        assert!(!bridge.owns_contribution_id("some-other-overlay"));
    }

    #[test]
    fn clear_blocks_stroke_to_fill_and_marker_to_edge_fallbacks() {
        let mut cleared_edge = ThemeStylePatch::default().with_fill(solid("#ef4444"));
        cleared_edge.stroke.paint = Specified::Clear;
        let cleared_edge_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, cleared_edge)),
        );
        let cleared_edge = parse(&cleared_edge_spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&cleared_edge), 0);
        assert_ne!(
            cleared_edge
                .effective_config
                .get_str("themeVariables.lineColor"),
            Some("#ef4444")
        );

        let edge = ThemeStylePatch::default().with_stroke(solid("#22c55e"));
        let mut marker = ThemeStylePatch::default();
        marker.stroke.paint = Specified::Clear;
        let cleared_marker_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(ThemeRule::new(ThemeTarget::Edge, edge))
                .with_rule(ThemeRule::new(ThemeTarget::Marker, marker)),
        );
        let cleared_marker = parse(&cleared_marker_spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&cleared_marker), 0);
        assert_ne!(
            cleared_marker
                .effective_config
                .get_str("themeVariables.arrowheadColor"),
            Some("#22c55e")
        );
    }

    #[test]
    fn unsupported_marker_paint_blocks_edge_fallback_outside_class() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::BLOCK,
        ] {
            let edge = ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_stroke(solid("#22c55e")),
            )
            .with_variant(ThemeVariant::Default)
            .for_family(family);
            let mut marker_style = ThemeStylePatch::default();
            marker_style.stroke.paint = Specified::Clear;
            let marker = ThemeRule::new(ThemeTarget::Marker, marker_style).for_family(family);
            let spec = DiagramThemeSpec::new()
                .with_styles(ThemeRuleSet::default().with_rule(edge).with_rule(marker));
            let artifact = bridge(&spec).compile_for_family(family);

            let edge_contribution = format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str(),
            );
            assert!(
                !artifact.contribution_ids.contains(&edge_contribution),
                "edge stroke bridge ownership must follow the family matrix for {family:?}",
            );
            assert!(!artifact.contribution_ids.iter().any(|id| {
                id == &format!(
                    "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                    family.as_str(),
                )
            }));
        }
    }

    #[test]
    fn typed_flowchart_cluster_paint_does_not_create_legacy_assignments() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid("#ef4444"))
                        .with_stroke(solid("#2563eb")),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        let parsed = parse(&spec, "flowchart TD\nsubgraph Group\nA\nend\n");
        let baseline = parse(
            &DiagramThemeSpec::default(),
            "flowchart TD\nsubgraph Group\nA\nend\n",
        );

        assert_eq!(fallback_contribution_count(&parsed), 0);
        for path in [
            "themeVariables.clusterBkg",
            "themeVariables.secondaryColor",
            "themeVariables.clusterBorder",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Cluster paint must not mutate legacy `{path}`"
            );
        }
    }

    #[test]
    fn edge_paint_has_no_legacy_projection_and_block_keeps_explicit_markers() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::CLASS,
            DiagramFamilyId::BLOCK,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for paint in [CanvasPaint::Transparent, solid("#22c55e")] {
                    let mut edge = ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(paint),
                    )
                    .for_family(family);
                    if let Some(variant) = variant {
                        edge = edge.with_variant(variant);
                    }
                    let styles = ThemeRuleSet::default().with_rule(edge);
                    let spec = DiagramThemeSpec::new().with_styles(styles.clone());
                    let artifact = bridge(&spec).compile_for_family(family);
                    let typed = matches!(
                        family,
                        DiagramFamilyId::FLOWCHART
                            | DiagramFamilyId::SWIMLANE
                            | DiagramFamilyId::CLASS
                            | DiagramFamilyId::BLOCK
                    );
                    for projection in ["edge.stroke", "marker.paint-from-edge"] {
                        assert_eq!(
                            artifact.contribution_ids.contains(&format!(
                                "{CONTRIBUTION_ID_PREFIX}{family}.{projection}"
                            )),
                            !typed,
                            "{family}/{variant:?}/{projection}"
                        );
                    }
                    if typed {
                        assert!(artifact.overlay.is_empty());
                        assert!(artifact.contribution_ids.is_empty());
                        for marker_patch in [
                            ThemeStylePatch::default().with_fill(solid("#2468ac")),
                            ThemeStylePatch::default().with_stroke(solid("#2468ac")),
                        ] {
                            let spec = DiagramThemeSpec::new().with_styles(
                                styles.clone().with_rule(
                                    ThemeRule::new(ThemeTarget::Marker, marker_patch)
                                        .for_family(family),
                                ),
                            );
                            let artifact = bridge(&spec).compile_for_family(family);
                            assert!(artifact.contribution_ids.is_empty());
                            assert!(artifact.overlay.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn typed_node_family_edge_stroke_retires_legacy_marker_fallback() {
        for family in [
            DiagramFamilyId::BLOCK,
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::CLASS,
        ] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                    )
                    .for_family(family),
                ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert!(artifact.overlay.is_empty());
            assert!(artifact.contribution_ids.is_empty());
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                family.as_str()
            )));
        }
    }

    #[test]
    fn class_default_node_surfaces_retire_their_legacy_projections() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(solid("#112233"))
                            .with_stroke(solid("#445566")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid("#ddeeff")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::CLASS),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::CLASS);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());

        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");
        for path in [
            "themeVariables.primaryColor",
            "themeVariables.mainBkg",
            "themeVariables.primaryBorderColor",
            "themeVariables.nodeBorder",
            "themeVariables.primaryTextColor",
            "themeVariables.classText",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Class Default node surface must not mutate legacy `{path}`"
            );
        }
    }

    #[test]
    fn class_default_node_surfaces_keep_all_typed_projections_out_of_the_bridge() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#e2e8f0")),
                    )
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(solid("#112233"))
                            .with_stroke(solid("#445566")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid("#ddeeff")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::CLASS),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::CLASS);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        for projection in [
            ThemeRouteCutoverProjection::NodeFill,
            ThemeRouteCutoverProjection::NodeStroke,
            ThemeRouteCutoverProjection::NodeLabelFill,
        ] {
            assert!(
                !bridge.owns_contribution_id(&format!(
                    "{CONTRIBUTION_ID_PREFIX}class.{}",
                    projection.contribution_id()
                )),
                "typed Class Default {:?} must not regain a legacy projection",
                projection,
            );
        }

        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.titleColor"),
            baseline
                .effective_config
                .get_str("themeVariables.titleColor")
        );
        for path in [
            "themeVariables.primaryColor",
            "themeVariables.mainBkg",
            "themeVariables.primaryBorderColor",
            "themeVariables.nodeBorder",
            "themeVariables.primaryTextColor",
            "themeVariables.classText",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Class Default node surface must not mutate legacy `{path}`"
            );
        }

        let evidence = theme_parse_evidence(&parsed);
        assert_eq!(evidence.fallback_contribution_count(), 0);
        assert!(evidence.fallback_contributions().next().is_none());
    }

    #[test]
    fn unsupported_class_marker_rule_does_not_recreate_edge_bridge_assignments() {
        for variant in [None, Some(ThemeVariant::Default)] {
            for patch in [
                ThemeStylePatch::default().with_fill(solid("#22c55e")),
                ThemeStylePatch::default().with_stroke(solid("#22c55e")),
            ] {
                let mut edge =
                    ThemeRule::new(ThemeTarget::Edge, patch).for_family(DiagramFamilyId::CLASS);
                if let Some(variant) = variant {
                    edge = edge.with_variant(variant);
                }
                let marker = ThemeRule::new(
                    ThemeTarget::Marker,
                    ThemeStylePatch::default().with_stroke(solid("#ef4444")),
                )
                .for_family(DiagramFamilyId::CLASS);
                let spec = DiagramThemeSpec::new()
                    .with_styles(ThemeRuleSet::default().with_rule(edge).with_rule(marker));
                let parsed = parse(&spec, "classDiagram\nA *-- B\n");
                let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nA *-- B\n");
                assert_eq!(
                    parsed.effective_config.as_value(),
                    baseline.effective_config.as_value()
                );
                assert_eq!(fallback_contribution_count(&parsed), 0);
            }
        }
    }

    #[test]
    fn generic_text_preserves_only_class_cluster_label_assignments() {
        for family in [DiagramFamilyId::BLOCK, DiagramFamilyId::CLASS] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#2468ac")),
                    )
                    .for_family(family),
                ),
            );
            let artifact = bridge(&spec).compile_for_family(family);
            assert!(!artifact.contribution_ids.contains(&format!(
                "{CONTRIBUTION_ID_PREFIX}{family}.cluster-label.fill"
            )));
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{CONTRIBUTION_ID_PREFIX}{family}.node-label.fill"))
            );
        }
    }

    #[test]
    fn node_family_cluster_label_rules_do_not_recreate_legacy_projections() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::CLASS,
            DiagramFamilyId::BLOCK,
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for paint in [CanvasPaint::Transparent, solid("#2468ac")] {
                    let mut rule = ThemeRule::new(
                        ThemeTarget::ClusterLabel,
                        ThemeStylePatch::default().with_fill(paint),
                    )
                    .for_family(family);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let spec = DiagramThemeSpec::new()
                        .with_styles(ThemeRuleSet::default().with_rule(rule));
                    let artifact = bridge(&spec).compile_for_family(family);
                    let typed = matches!(
                        family,
                        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
                    );
                    let contribution =
                        format!("{CONTRIBUTION_ID_PREFIX}{family}.cluster-label.fill");
                    assert!(
                        !artifact.contribution_ids.contains(&contribution),
                        "family={family}, variant={variant:?}",
                    );
                    if typed {
                        assert!(artifact.overlay.is_empty());
                        assert!(artifact.contribution_ids.is_empty());
                        let source = "flowchart TD\nsubgraph Group\nA\nend\n";
                        let resolver = bridge(&spec);
                        let plan = ThemeCompatibilityPlan::try_new(
                            [0x5a; 32],
                            MermaidConfig::empty_object(),
                            move |family, control| resolver.overlay_for_family(family, control),
                        )
                        .unwrap();
                        for theme in ["default", "base", "dark"] {
                            let mut value = serde_json::json!({"theme": theme});
                            if family == DiagramFamilyId::SWIMLANE {
                                value["layout"] = serde_json::json!("swimlane");
                            }
                            let config = MermaidConfig::from_value(value);
                            let baseline = merman_core::Engine::new()
                                .with_site_config(config.clone())
                                .parse_metadata_sync(source)
                                .unwrap();
                            let themed = install_theme_compatibility(
                                merman_core::Engine::new().with_site_config(config),
                                &plan,
                            )
                            .parse_metadata_sync(source)
                            .unwrap();
                            assert_eq!(
                                themed.effective_config.as_value(),
                                baseline.effective_config.as_value()
                            );
                            assert_eq!(fallback_contribution_count(&themed), 0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn node_family_title_cutover_preserves_other_legacy_title_consumers() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::CLASS,
            DiagramFamilyId::BLOCK,
        ] {
            for target in [ThemeTarget::Title, ThemeTarget::Text] {
                for variant in [None, Some(ThemeVariant::Default)] {
                    let mut rule = ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(solid("#2468ac")),
                    )
                    .for_family(family);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let spec = DiagramThemeSpec::new()
                        .with_styles(ThemeRuleSet::default().with_rule(rule));
                    let artifact = bridge(&spec).compile_for_family(family);
                    assert_eq!(
                        artifact
                            .contribution_ids
                            .contains(&format!("{CONTRIBUTION_ID_PREFIX}{family}.title.fill")),
                        false,
                        "family={family}, target={target:?}, variant={variant:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn class_title_fill_leaves_namespace_label_config_to_the_typed_writer() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(solid("#e2e8f0")),
                )
                .for_family(DiagramFamilyId::CLASS),
            ),
        );
        let parsed = parse(
            &spec,
            "classDiagram\nnamespace Platform {\n  class Runtime\n}\n",
        );

        assert_eq!(
            parsed.effective_config.get_str("themeVariables.titleColor"),
            parse(
                &DiagramThemeSpec::default(),
                "classDiagram\nnamespace Platform {\nclass Runtime\n}\n"
            )
            .effective_config
            .get_str("themeVariables.titleColor"),
        );
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn class_terminal_less_paint_rules_do_not_create_legacy_assignments() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default()
                            .with_fill(solid("#0f172a"))
                            .with_stroke(solid("#22c55e")),
                    )
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::ClusterLabel,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .with_variant(ThemeVariant::Odd)
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#475569")),
                    )
                    .with_variant(ThemeVariant::Even)
                    .for_family(DiagramFamilyId::CLASS),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::CLASS);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());

        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");
        for path in [
            "themeVariables.arrowheadColor",
            "themeVariables.secondaryTextColor",
            "themeVariables.tertiaryTextColor",
            "themeVariables.attributeBackgroundColorOdd",
            "themeVariables.attributeBackgroundColorEven",
            "themeVariables.rowOdd",
            "themeVariables.rowEven",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "unsupported Class route must not mutate `{path}`"
            );
        }
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn typed_sequence_message_paints_suppress_the_shared_legacy_projection() {
        for (facet, patch) in [
            (
                "fill",
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            ),
            (
                "stroke",
                ThemeStylePatch::default().with_stroke(solid("#2563eb")),
            ),
        ] {
            for variant in [None, Some(ThemeVariant::Default)] {
                let mut rule = ThemeRule::new(ThemeTarget::Message, patch.clone())
                    .for_family(DiagramFamilyId::SEQUENCE);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let spec =
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
                let bridge = bridge(&spec);
                let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

                assert!(artifact.overlay.is_empty(), "{facet} {variant:?}");
                assert!(artifact.contribution_ids.is_empty(), "{facet} {variant:?}");
                assert!(
                    !bridge.owns_contribution_id(
                        "merman.legacy-family-theme.v1.sequence.message.stroke"
                    ),
                    "{facet} {variant:?}"
                );

                let parsed = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");
                let baseline = parse(
                    &DiagramThemeSpec::default(),
                    "sequenceDiagram\nAlice->>Bob: Hello\n",
                );
                assert_eq!(
                    fallback_contribution_count(&parsed),
                    0,
                    "typed Message {facet} must not create a compatibility contribution"
                );
                assert_eq!(
                    parsed
                        .effective_config
                        .get_str("themeVariables.signalColor"),
                    baseline
                        .effective_config
                        .get_str("themeVariables.signalColor"),
                    "typed Message {facet} must not mutate the legacy signalColor"
                );
            }
        }
    }

    #[test]
    fn typed_sequence_lifeline_paint_suppresses_its_shared_legacy_projection() {
        for style in [
            ThemeStylePatch::default().with_fill(solid("#ef4444")),
            ThemeStylePatch::default().with_stroke(solid("#2563eb")),
            ThemeStylePatch::default()
                .with_fill(solid("#ef4444"))
                .with_stroke(solid("#2563eb")),
        ] {
            let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Lifeline, style).for_family(DiagramFamilyId::SEQUENCE),
            ));
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(artifact.overlay.is_empty());
            assert!(artifact.contribution_ids.is_empty());
            assert!(
                !bridge
                    .owns_contribution_id("merman.legacy-family-theme.v1.sequence.lifeline.stroke")
            );
        }
    }

    #[test]
    fn typed_sequence_role_paints_suppress_unqualified_and_default_legacy_projections() {
        for (target, style, contribution_id) in [
            (
                ThemeTarget::ActorLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.actor-label.fill",
            ),
            (
                ThemeTarget::MessageLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.message-label.fill",
            ),
            (
                ThemeTarget::Loop,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.loop.fill",
            ),
            (
                ThemeTarget::Loop,
                ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                "merman.legacy-family-theme.v1.sequence.loop.stroke",
            ),
            (
                ThemeTarget::LoopLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.loop-label.fill",
            ),
            (
                ThemeTarget::NoteLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.note-label.fill",
            ),
        ] {
            let direct_spec =
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(target, style.clone()).for_family(DiagramFamilyId::SEQUENCE),
                ));
            let direct_bridge = bridge(&direct_spec);
            let direct = direct_bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(!direct.contribution_ids.contains(contribution_id));
            assert!(!direct_bridge.owns_contribution_id(contribution_id));

            let default_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(target, style)
                        .with_variant(ThemeVariant::Default)
                        .for_family(DiagramFamilyId::SEQUENCE),
                ),
            );
            let default_bridge = bridge(&default_spec);
            let default_overlay = default_bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(!default_overlay.contribution_ids.contains(contribution_id));
            assert!(!default_bridge.owns_contribution_id(contribution_id));
        }
    }

    #[test]
    fn explicit_marker_paint_has_no_bridge_owner_when_edge_stroke_is_typed() {
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                        )
                        .for_family(family),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Marker,
                            ThemeStylePatch::default().with_stroke(solid("#d946ef")),
                        )
                        .for_family(family),
                    ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert!(artifact.contribution_ids.is_empty());
            assert!(artifact.overlay.is_empty());
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
        }
    }

    #[test]
    fn contribution_ownership_is_exact_and_recipe_deterministic() {
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )));
        let bridge = bridge(&spec);
        assert_eq!(bridge.cached_family_count(), 0);

        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill"));
        assert_eq!(bridge.cached_family_count(), 1);
        assert!(
            !bridge
                .owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill.forged")
        );
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.node.fill"));
    }

    #[test]
    fn overlay_builder_rejects_duplicate_assignments_without_erasing_prior_state() {
        let mut builder = OverlayBuilder::new(DiagramFamilyId::FLOWCHART);
        let mut direct = FamilyContributions::new();
        direct.add_theme_variables("direct", [("primaryColor", Some("#ef4444".to_string()))]);
        direct
            .finish_into(&mut builder)
            .expect("first contribution is valid");

        let mut duplicate = FamilyContributions::new();
        duplicate.add_theme_variables("duplicate", [("primaryColor", Some("#22c55e".to_string()))]);
        let error = duplicate
            .finish_into(&mut builder)
            .expect_err("duplicate assignment must fail closed");
        assert!(error.to_string().contains("duplicate"));

        let artifact = builder.finish();
        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.flowchart.direct")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.flowchart.duplicate")
        );
    }

    #[test]
    fn overlay_builder_propagates_empty_patch_and_duplicate_id_errors() {
        let mut builder = OverlayBuilder::new(DiagramFamilyId::FLOWCHART);
        let empty_error = builder
            .push("empty", Map::new())
            .expect_err("empty compatibility patch must fail closed");
        assert!(empty_error.to_string().contains("at least one scalar"));

        let mut first = Map::new();
        first.insert(
            "fontFamily".to_string(),
            Value::String("FirstFont".to_string()),
        );
        builder
            .push("typography", first)
            .expect("first contribution id is valid");

        let mut duplicate_id = Map::new();
        duplicate_id.insert("fontSize".to_string(), Value::String("24px".to_string()));
        let duplicate_error = builder
            .push("typography", duplicate_id)
            .expect_err("duplicate contribution id must fail closed");
        assert!(duplicate_error.to_string().contains("duplicate"));

        let artifact = builder.finish();
        assert_eq!(
            artifact.contribution_ids,
            BTreeSet::from(["merman.legacy-family-theme.v1.flowchart.typography".to_string()])
        );
    }

    #[test]
    fn bridge_does_not_inject_a_global_base_theme() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::FLOWCHART);
        assert!(
            !artifact
                .contribution_ids
                .iter()
                .any(|id| id.ends_with(".theme-base"))
        );
        let parsed = parse(&spec, "flowchart LR\nA --> B\n");
        assert_ne!(parsed.effective_config.get_str("theme"), Some("base"));
    }

    #[test]
    fn unsupported_paint_does_not_create_a_direct_assignment() {
        let gradient = CanvasPaint::LinearGradient(
            super::super::LinearGradient::new(
                0.0,
                [
                    super::super::GradientStop::new(
                        0.0,
                        super::super::ThemeColorValue::parse("#ef4444").unwrap(),
                    )
                    .unwrap(),
                    super::super::GradientStop::new(
                        1.0,
                        super::super::ThemeColorValue::parse("#3b82f6").unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .expect("valid test gradient"),
        );
        let mut edge = ThemeStylePatch::default().with_fill(solid("#22c55e"));
        edge.stroke.paint = Specified::Value(gradient);
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, edge)),
        );
        let bridge = bridge(&spec);
        bridge.compile_for_family(DiagramFamilyId::FLOWCHART);
        assert!(
            !bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.edge.stroke")
        );
    }

    #[test]
    fn family_clear_blocks_an_inherited_public_fill() {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        clear.stroke.paint = Specified::Clear;
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                ))
                .with_rule(
                    ThemeRule::new(ThemeTarget::Node, clear).for_family(DiagramFamilyId::FLOWCHART),
                ),
        );

        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        assert_ne!(
            flowchart
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#f8fafc")
        );
    }

    #[test]
    fn task_and_requirement_families_use_their_own_programs() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f8fafc"))
                            .with_stroke(solid("#94a3b8")),
                    )
                    .for_family(DiagramFamilyId::GANTT),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f1f5f9"))
                            .with_stroke(solid("#64748b")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Active),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default().with_stroke(solid("#059669")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Success),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default().with_stroke(solid("#dc2626")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Error),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f8fafc"))
                            .with_stroke(solid("#94a3b8")),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Relation,
                        ThemeStylePatch::default().with_stroke(solid("#64748b")),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
        );
        let gantt_baseline = parse(&DiagramThemeSpec::new(), GANTT_FIXTURE);
        let gantt = parse(&spec, GANTT_FIXTURE);
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.taskBkgColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.taskBkgColor")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.activeTaskBkgColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.activeTaskBkgColor")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.taskBorderColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.taskBorderColor")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.doneTaskBorderColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.doneTaskBorderColor")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.critBorderColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.critBorderColor")
        );
        assert_eq!(fallback_contribution_count(&gantt), 0);
        let gantt_bridge = bridge(&spec).compile_for_family(DiagramFamilyId::GANTT);
        assert!(gantt_bridge.overlay.is_empty());
        assert!(gantt_bridge.contribution_ids.is_empty());

        let requirement_baseline = parse(&DiagramThemeSpec::new(), REQUIREMENT_FIXTURE);
        let requirement = parse(&spec, REQUIREMENT_FIXTURE);
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.requirementBackground"),
            requirement_baseline
                .effective_config
                .get_str("themeVariables.requirementBackground")
        );
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.requirementBorderColor"),
            requirement_baseline
                .effective_config
                .get_str("themeVariables.requirementBorderColor")
        );
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.relationColor"),
            requirement_baseline
                .effective_config
                .get_str("themeVariables.relationColor")
        );
        assert_eq!(fallback_contribution_count(&requirement), 0);
        let requirement_bridge = bridge(&spec).compile_for_family(DiagramFamilyId::REQUIREMENT);
        assert!(
            !requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.requirement.fill")
        );
        assert!(
            !requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.requirement.paint")
        );
        assert!(
            !requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.relation.paint")
        );
    }

    #[test]
    fn gantt_warning_lines_are_owned_by_the_typed_writer() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Task,
                    ThemeStylePatch::default().with_stroke(solid("#d97706")),
                )
                .for_family(DiagramFamilyId::GANTT)
                .with_variant(ThemeVariant::Warning),
            ),
        );
        let parsed = parse(&spec, GANTT_FIXTURE);
        let compiled = bridge(&spec).compile_for_family(DiagramFamilyId::GANTT);

        let baseline = parse(&DiagramThemeSpec::new(), GANTT_FIXTURE);
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.todayLineColor"),
            baseline
                .effective_config
                .get_str("themeVariables.todayLineColor")
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.vertLineColor"),
            baseline
                .effective_config
                .get_str("themeVariables.vertLineColor")
        );
        assert_eq!(fallback_contribution_count(&parsed), 0);
        assert!(compiled.overlay.is_empty());
        assert!(compiled.contribution_ids.is_empty());
    }

    #[test]
    fn explicit_default_requirement_paint_skips_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Requirement,
                    ThemeStylePatch::default()
                        .with_fill(solid("#f8fafc"))
                        .with_stroke(solid("#94a3b8")),
                )
                .for_family(DiagramFamilyId::REQUIREMENT)
                .with_variant(ThemeVariant::Default),
            ),
        );
        let baseline = parse(&DiagramThemeSpec::new(), REQUIREMENT_FIXTURE);
        let parsed = parse(&spec, REQUIREMENT_FIXTURE);
        let compiled = bridge(&spec).compile_for_family(DiagramFamilyId::REQUIREMENT);

        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.requirementBackground"),
            baseline
                .effective_config
                .get_str("themeVariables.requirementBackground")
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.requirementBorderColor"),
            baseline
                .effective_config
                .get_str("themeVariables.requirementBorderColor")
        );
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.nodeBorder"),
            baseline
                .effective_config
                .get_str("themeVariables.nodeBorder")
        );
        assert!(compiled.overlay.is_empty());
        assert!(compiled.contribution_ids.is_empty());
    }

    #[test]
    fn unsupported_requirement_table_paint_does_not_mutate_row_colors() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .with_variant(ThemeVariant::Odd)
                    .for_family(DiagramFamilyId::REQUIREMENT),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#e2e8f0")),
                    )
                    .with_variant(ThemeVariant::Even)
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
        );
        let parsed = parse(&spec, REQUIREMENT_FIXTURE);
        let baseline = parse(&DiagramThemeSpec::default(), REQUIREMENT_FIXTURE);
        let compiled = bridge(&spec).compile_for_family(DiagramFamilyId::REQUIREMENT);

        for path in ["themeVariables.rowOdd", "themeVariables.rowEven"] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "unsupported Requirement table paint must not mutate `{path}`"
            );
        }
        assert!(compiled.overlay.is_empty());
        assert!(compiled.contribution_ids.is_empty());
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn direct_pie_xy_and_radar_palettes_do_not_create_bridge_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#2563eb").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#16a34a").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#d97706").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#9333ea").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_ordinal_palette(ThemeTarget::PieSlice, palette.clone())
                .with_ordinal_palette(ThemeTarget::ChartSeries, palette.clone())
                .with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        );

        let baseline = parse(&DiagramThemeSpec::new(), PIE_FIXTURE);
        let pie = parse(&spec, PIE_FIXTURE);
        assert_eq!(
            pie.effective_config.get_str("themeVariables.pie1"),
            baseline.effective_config.get_str("themeVariables.pie1")
        );
        assert_eq!(fallback_contribution_count(&pie), 0);

        let xy = bridge(&spec).compile_for_family(DiagramFamilyId::XY_CHART);
        assert!(
            !xy.contribution_ids
                .contains("merman.legacy-family-theme.v1.xychart.series.palette")
        );

        let radar = bridge(&spec).compile_for_family(DiagramFamilyId::RADAR);
        assert!(
            !radar
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.radar.series.palette")
        );

        let journey = bridge(&spec).compile_for_family(DiagramFamilyId::JOURNEY);
        assert!(
            !journey
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.task.palette")
        );
    }

    #[test]
    fn xychart_base_typography_does_not_create_a_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                DiagramFamilyId::XY_CHART,
                TextStyle::default()
                    .with_font_stack(
                        FontStack::single("XY Chart Phantom Font")
                            .expect("valid XY Chart font stack"),
                    )
                    .with_font_size_px(24.0)
                    .expect("valid XY Chart font size"),
            ),
        );

        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::XY_CHART);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn direct_timeline_event_palette_does_not_create_bridge_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#2563eb").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#16a34a").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_ordinal_palette(ThemeTarget::TimelineEvent, palette)
                .with_rule(ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(
                        super::super::CanvasPaint::solid("#111827").expect("valid text color"),
                    ),
                )),
        );

        let timeline = bridge(&spec).compile_for_family(DiagramFamilyId::TIMELINE);
        assert!(
            !timeline
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.timeline.event.palette")
        );
    }

    #[test]
    fn timeline_typed_paint_has_no_family_bridge_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::TimelineEvent,
                        ThemeStylePatch::default()
                            .with_fill(solid("#123456"))
                            .with_stroke(solid("#654321")),
                    )
                    .for_family(DiagramFamilyId::TIMELINE),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#111827")),
                    )
                    .for_family(DiagramFamilyId::TIMELINE),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(solid("#9333ea")),
                    )
                    .for_family(DiagramFamilyId::TIMELINE),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::TIMELINE);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.timeline.event.fill")
        );
        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.timeline.event.text")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.timeline.event.paint-text")
        );
        assert!(!bridge.owns_contribution_id(
            ThemeRouteCutoverProjection::TimelineEventFill.contribution_id()
        ));
        assert!(!bridge.owns_contribution_id(
            ThemeRouteCutoverProjection::TimelineEventStroke.contribution_id()
        ));
    }
    #[test]
    fn quadrant_paint_cutovers_suppress_each_legacy_projection() {
        for target in [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Axis] {
            for variant in [None, Some(ThemeVariant::Default)] {
                for paint in [solid("#13579b"), CanvasPaint::Transparent] {
                    let mut rule =
                        ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint));
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let spec = DiagramThemeSpec::new()
                        .with_styles(ThemeRuleSet::default().with_rule(rule));
                    let bridge = bridge(&spec);
                    let quadrant = bridge.compile_for_family(DiagramFamilyId::QUADRANT_CHART);
                    assert!(quadrant.overlay.is_empty());
                    assert!(quadrant.contribution_ids.is_empty());
                    let xy = bridge.compile_for_family(DiagramFamilyId::XY_CHART);
                    assert!(xy.overlay.is_empty());
                    assert!(xy.contribution_ids.is_empty());
                }
            }
        }
    }
}
