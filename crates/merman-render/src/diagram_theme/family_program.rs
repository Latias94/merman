use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use crate::render_family::RenderFamilyKind;
use crate::resources::{OperationWorkMeter, ResourceLimitExceeded};

use super::DiagramThemeSpec;
use super::family_mechanism_matrix::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRoute, FamilyThemeRuleFacet,
    compile_base_typography_routes, compile_effect_binding_route, compile_ordinal_palette_route,
    compile_rule_routes,
};
use super::resolved::{ResolvedThemeStyle, ThemeRuleOrigin};
use super::semantic::{OrdinalPalette, ThemeTarget, ThemeVariant};
use super::typography::TextStyle;

/// One immutable, family-local interpretation of a validated theme recipe.
///
/// Family, target, and variant matching are compiled once. Only ordinal selectors remain dynamic
/// because they depend on the concrete diagram element being styled.
#[derive(Debug)]
pub(super) struct FamilyThemeProgram {
    spec: Arc<DiagramThemeSpec>,
    base_typography: TextStyle,
    rule_indices: Box<[usize]>,
    slots: BTreeMap<ThemeTarget, FamilyRuleSlot>,
    ordinal_palette_indices: BTreeMap<ThemeTarget, usize>,
    effect_binding_indices: Box<[usize]>,
    mechanism_routes: Box<[FamilyThemeRoute]>,
}

/// Per-recipe cache for the immutable family programs consumed by both typed adapters and the
/// temporary Mermaid compatibility bridge.
#[derive(Debug)]
pub(super) struct FamilyThemeProgramCache {
    spec: Arc<DiagramThemeSpec>,
    programs: Mutex<HashMap<RenderFamilyKind, Arc<FamilyThemeProgram>>>,
}

impl FamilyThemeProgramCache {
    pub(super) fn new(spec: Arc<DiagramThemeSpec>) -> Self {
        Self {
            spec,
            programs: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn get_or_compile(&self, family: RenderFamilyKind) -> Arc<FamilyThemeProgram> {
        let mut programs = self
            .programs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        programs
            .entry(family)
            .or_insert_with(|| {
                Arc::new(FamilyThemeProgram::compile(Arc::clone(&self.spec), family))
            })
            .clone()
    }

    #[cfg(test)]
    pub(super) fn contains(&self, family: RenderFamilyKind) -> bool {
        self.programs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&family)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.programs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

#[derive(Debug)]
struct FamilyRuleSlot {
    common_static: ResolvedThemeStyle,
    variant_static: BTreeMap<ThemeVariant, ResolvedThemeStyle>,
    common_ordinal: Vec<ThemeRuleOrigin>,
    variant_ordinal: BTreeMap<ThemeVariant, Vec<ThemeRuleOrigin>>,
}

impl FamilyThemeProgram {
    pub(super) fn compile(spec: Arc<DiagramThemeSpec>, family: RenderFamilyKind) -> Self {
        let base_typography = spec.typography().family_style(family).clone();
        let mut rule_indices = Vec::new();
        let mut slots = BTreeMap::<ThemeTarget, FamilyRuleSlot>::new();
        let mut mechanism_routes = compile_base_typography_routes(family, &base_typography);

        for (rule_index, rule) in spec.styles().rules().iter().enumerate() {
            if rule.target() == ThemeTarget::Canvas
                || !rule.target().valid_for(family)
                || rule.family().is_some_and(|expected| expected != family)
            {
                continue;
            }

            rule_indices.push(rule_index);
            mechanism_routes.extend(compile_rule_routes(family, rule_index, rule));
            slots
                .entry(rule.target())
                .or_insert_with(|| FamilyRuleSlot::new(base_typography.clone()))
                .compile_rule(ThemeRuleOrigin::new(rule_index, rule), rule.style());
        }

        let mut ordinal_palette_indices = BTreeMap::new();
        for (index, (target, _)) in spec.styles().ordinal_palettes().iter().enumerate() {
            if *target == ThemeTarget::Canvas || !target.valid_for(family) {
                continue;
            }
            ordinal_palette_indices.insert(*target, index);
            mechanism_routes.push(compile_ordinal_palette_route(family, *target));
        }
        let mut effect_binding_indices = Vec::new();
        for (index, binding) in spec.effects().bindings().iter().enumerate() {
            if binding.target() == ThemeTarget::Canvas || !binding.target().valid_for(family) {
                continue;
            }
            effect_binding_indices.push(index);
            mechanism_routes.push(compile_effect_binding_route(
                family,
                index,
                binding.target(),
            ));
        }

        Self {
            spec,
            base_typography,
            rule_indices: rule_indices.into_boxed_slice(),
            slots,
            ordinal_palette_indices,
            effect_binding_indices: effect_binding_indices.into_boxed_slice(),
            mechanism_routes: mechanism_routes.into_boxed_slice(),
        }
    }

    pub(super) const fn base_typography(&self) -> &TextStyle {
        &self.base_typography
    }

    #[cfg(test)]
    pub(super) fn has_family_style_requirements(&self) -> bool {
        self.base_typography != TextStyle::default()
            || !self.rule_indices.is_empty()
            || !self.ordinal_palette_indices.is_empty()
            || !self.effect_binding_indices.is_empty()
    }

    pub(super) fn rule_indices(&self) -> &[usize] {
        &self.rule_indices
    }

    pub(super) fn ordinal_palette_targets(&self) -> impl Iterator<Item = ThemeTarget> + '_ {
        self.ordinal_palette_indices.keys().copied()
    }

    pub(super) fn ordinal_palette_index(&self, target: ThemeTarget) -> Option<usize> {
        self.ordinal_palette_indices.get(&target).copied()
    }

    pub(super) fn ordinal_palette(&self, target: ThemeTarget) -> Option<&OrdinalPalette> {
        let index = self.ordinal_palette_index(target)?;
        Some(&self.spec.styles().ordinal_palettes()[index].1)
    }

    pub(super) fn effect_binding_indices(&self) -> &[usize] {
        &self.effect_binding_indices
    }

    pub(super) fn has_legacy_base_typography(
        &self,
        property: super::resolved::ThemeTypographyProperty,
    ) -> bool {
        self.has_route(
            FamilyThemeMechanism::BaseTypography(property),
            FamilyThemeDisposition::LegacyCompatibility,
        )
    }

    pub(super) fn has_legacy_rule_facet(
        &self,
        rule_index: usize,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.rule_facet_disposition(rule_index, facet)
            == Some(FamilyThemeDisposition::LegacyCompatibility)
    }

    pub(super) fn rule_facet_disposition(
        &self,
        rule_index: usize,
        facet: FamilyThemeRuleFacet,
    ) -> Option<FamilyThemeDisposition> {
        self.mechanism_routes.iter().find_map(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    rule_index: candidate,
                    facet: candidate_facet,
                    ..
                } if candidate == rule_index && candidate_facet == facet
            )
            .then_some(route.disposition())
        })
    }

    pub(super) fn has_mechanism_routes(&self) -> bool {
        !self.mechanism_routes.is_empty()
    }

    pub(super) fn mechanism_routes(&self) -> &[FamilyThemeRoute] {
        &self.mechanism_routes
    }

    pub(super) fn has_legacy_ordinal_palette(&self, target: ThemeTarget) -> bool {
        self.has_route(
            FamilyThemeMechanism::OrdinalPalette { target },
            FamilyThemeDisposition::LegacyCompatibility,
        )
    }

    fn has_route(
        &self,
        mechanism: FamilyThemeMechanism,
        disposition: FamilyThemeDisposition,
    ) -> bool {
        self.mechanism_routes
            .iter()
            .any(|route| route.mechanism() == mechanism && route.disposition() == disposition)
    }

    pub(super) fn resolve_style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        self.resolve_style_internal(target, variant, ordinal, None)
            .expect("unmetered family theme resolution cannot fail")
    }

    pub(super) fn resolve_style_with_work_meter(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, ResourceLimitExceeded> {
        self.resolve_style_internal(target, variant, ordinal, Some(work_meter))
    }

    fn resolve_style_internal(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ResolvedThemeStyle, ResourceLimitExceeded> {
        let mut resolved = ResolvedThemeStyle::new(self.base_typography.clone());
        if let Some(slot) = self.slots.get(&target) {
            slot.apply_static(&mut resolved, variant);
            charge_ordinal_candidates(work_meter, ordinal, slot.ordinal_candidate_count(variant))?;
            slot.apply_ordinal(&self.spec, &mut resolved, variant, ordinal);
        }
        Ok(resolved)
    }

    pub(super) fn resolve_text_style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        self.resolve_text_style_internal(target, variant, ordinal, None)
            .expect("unmetered family text theme resolution cannot fail")
    }

    pub(super) fn resolve_text_style_with_work_meter(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, ResourceLimitExceeded> {
        self.resolve_text_style_internal(target, variant, ordinal, Some(work_meter))
    }

    fn resolve_text_style_internal(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ResolvedThemeStyle, ResourceLimitExceeded> {
        let mut resolved = ResolvedThemeStyle::new(self.base_typography.clone());
        let ordinal_candidate_count = self.ordinal_candidate_count(ThemeTarget::Text, variant)
            + if target == ThemeTarget::Text {
                0
            } else {
                self.ordinal_candidate_count(target, variant)
            };
        charge_ordinal_candidates(work_meter, ordinal, ordinal_candidate_count)?;
        self.apply_text_target(ThemeTarget::Text, &mut resolved, variant, ordinal);
        if target != ThemeTarget::Text {
            self.apply_text_target(target, &mut resolved, variant, ordinal);
        }
        Ok(resolved)
    }

    fn ordinal_candidate_count(&self, target: ThemeTarget, variant: ThemeVariant) -> usize {
        self.slots
            .get(&target)
            .map_or(0, |slot| slot.ordinal_candidate_count(variant))
    }

    fn apply_text_target(
        &self,
        target: ThemeTarget,
        resolved: &mut ResolvedThemeStyle,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) {
        let Some(slot) = self.slots.get(&target) else {
            return;
        };
        slot.apply_static(resolved, variant);
        slot.apply_ordinal(&self.spec, resolved, variant, ordinal);
    }
}

impl FamilyRuleSlot {
    fn new(base_typography: TextStyle) -> Self {
        Self {
            common_static: ResolvedThemeStyle::new(base_typography),
            variant_static: BTreeMap::new(),
            common_ordinal: Vec::new(),
            variant_ordinal: BTreeMap::new(),
        }
    }

    fn compile_rule(&mut self, origin: ThemeRuleOrigin, patch: &super::semantic::ThemeStylePatch) {
        match (origin.variant(), origin.ordinal()) {
            (variant, Some(_)) => match variant {
                Some(variant) => self
                    .variant_ordinal
                    .entry(variant)
                    .or_default()
                    .push(origin),
                None => self.common_ordinal.push(origin),
            },
            (Some(variant), None) => self
                .variant_static
                .entry(variant)
                .or_insert_with(|| {
                    ResolvedThemeStyle::new(
                        self.common_static.typography_resolution().base().clone(),
                    )
                })
                .apply(patch, origin),
            (None, None) => self.common_static.apply(patch, origin),
        }
    }

    fn apply_static(&self, resolved: &mut ResolvedThemeStyle, variant: ThemeVariant) {
        resolved.merge_from(&self.common_static);
        if let Some(style) = self.variant_static.get(&variant) {
            resolved.merge_from(style);
        }
    }

    fn ordinal_candidate_count(&self, variant: ThemeVariant) -> usize {
        self.common_ordinal.len() + self.variant_ordinal.get(&variant).map_or(0, Vec::len)
    }

    fn apply_ordinal(
        &self,
        spec: &DiagramThemeSpec,
        resolved: &mut ResolvedThemeStyle,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) {
        let Some(ordinal) = ordinal else {
            return;
        };
        let rules = spec.styles().rules();
        for origin in self
            .common_ordinal
            .iter()
            .chain(self.variant_ordinal.get(&variant).into_iter().flatten())
        {
            let selector = origin
                .ordinal()
                .expect("compiled ordinal rule must retain its selector");
            if selector.matches(ordinal) {
                resolved.apply(rules[origin.rule_index()].style(), *origin);
            }
        }
    }
}

fn charge_ordinal_candidates(
    work_meter: Option<&OperationWorkMeter>,
    ordinal: Option<usize>,
    candidates: usize,
) -> Result<(), ResourceLimitExceeded> {
    if ordinal.is_some()
        && let Some(work_meter) = work_meter
    {
        work_meter.charge(candidates)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_reuses_the_same_program_for_one_recipe_family() {
        let spec = Arc::new(DiagramThemeSpec::default());
        let cache = FamilyThemeProgramCache::new(spec);

        let first = cache.get_or_compile(RenderFamilyKind::Flowchart);
        let second = cache.get_or_compile(RenderFamilyKind::Flowchart);
        let state = cache.get_or_compile(RenderFamilyKind::State);

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &state));
    }
}
