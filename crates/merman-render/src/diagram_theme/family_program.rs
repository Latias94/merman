use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use crate::DiagramFamilyId;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::DiagramThemeSpec;
use super::application::FamilyThemeMechanismKey;
use super::family_mechanism_matrix::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRoute, FamilyThemeRuleFacet,
    compile_base_typography_routes, compile_effect_binding_route, compile_ordinal_palette_route,
    compile_rule_routes,
};
use super::resolved::{ResolvedThemeStyle, ThemeRuleOrigin, ThemeStyleRuleWinners};
use super::semantic::{ThemeTarget, ThemeVariant};
use super::typography::TextStyle;

/// One immutable, family-local interpretation of a validated theme recipe.
///
/// Family, target, and variant matching are compiled once. Only ordinal selectors remain dynamic
/// because they depend on the concrete diagram element being styled.
#[derive(Debug)]
pub(crate) struct FamilyThemeProgram {
    spec: Arc<DiagramThemeSpec>,
    family: DiagramFamilyId,
    base_typography: TextStyle,
    rule_indices: Box<[usize]>,
    slots: BTreeMap<ThemeTarget, FamilyRuleSlot>,
    ordinal_palette_indices: BTreeMap<ThemeTarget, usize>,
    ordinal_palette_dispositions: BTreeMap<ThemeTarget, FamilyThemeDisposition>,
    effect_binding_indices: Box<[usize]>,
    mechanism_routes: Box<[FamilyThemeRoute]>,
    requested_mechanisms: Option<Arc<[FamilyThemeMechanismKey]>>,
    requested_index: BTreeSet<FamilyThemeMechanismKey>,
    rule_facet_dispositions: BTreeMap<(usize, FamilyThemeRuleFacet), FamilyThemeDisposition>,
}

/// Per-recipe cache for the immutable family programs consumed by typed adapters.
#[derive(Debug)]
pub(super) struct FamilyThemeProgramCache {
    spec: Arc<DiagramThemeSpec>,
    programs: Mutex<HashMap<DiagramFamilyId, Arc<FamilyThemeProgram>>>,
}

impl FamilyThemeProgramCache {
    pub(super) fn new(spec: Arc<DiagramThemeSpec>) -> Self {
        Self {
            spec,
            programs: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn get_or_compile(&self, family: DiagramFamilyId) -> Arc<FamilyThemeProgram> {
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
    pub(super) fn contains(&self, family: DiagramFamilyId) -> bool {
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

#[derive(Debug, Default)]
struct FamilyRuleSlot {
    common_static: ThemeStyleRuleWinners,
    variant_static: BTreeMap<ThemeVariant, ThemeStyleRuleWinners>,
    common_ordinal: Vec<ThemeRuleOrigin>,
    variant_ordinal: BTreeMap<ThemeVariant, Vec<ThemeRuleOrigin>>,
}

impl FamilyThemeProgram {
    pub(super) fn compile(spec: Arc<DiagramThemeSpec>, family: DiagramFamilyId) -> Self {
        let base_typography = spec.typography().family_style(family);
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
                .or_default()
                .compile_rule(ThemeRuleOrigin::new(rule_index, rule), rule.style());
        }

        let mut ordinal_palette_indices = BTreeMap::new();
        let mut ordinal_palette_dispositions = BTreeMap::new();
        for (index, (target, _)) in spec.styles().ordinal_palettes().iter().enumerate() {
            if *target == ThemeTarget::Canvas || !target.valid_for(family) {
                continue;
            }
            ordinal_palette_indices.insert(*target, index);
            let route = compile_ordinal_palette_route(family, *target);
            ordinal_palette_dispositions.insert(*target, route.disposition());
            mechanism_routes.push(route);
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
        let rule_facet_dispositions = mechanism_routes
            .iter()
            .filter_map(|route| match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index, facet, ..
                } => Some(((rule_index, facet), route.disposition())),
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => None,
            })
            .collect();

        let mut requested_mechanisms = Vec::new();
        let mut requested_index = BTreeSet::new();
        for route in mechanism_routes.iter().copied() {
            let key = mechanism_key(&spec, route);
            if requested_index.insert(key.clone()) {
                requested_mechanisms.push(key);
            }
        }
        let requested_mechanisms: Option<Arc<[FamilyThemeMechanismKey]>> =
            if requested_mechanisms.is_empty() {
                None
            } else {
                Some(Arc::from(requested_mechanisms))
            };

        Self {
            spec,
            family,
            base_typography,
            rule_indices: rule_indices.into_boxed_slice(),
            slots,
            ordinal_palette_indices,
            ordinal_palette_dispositions,
            effect_binding_indices: effect_binding_indices.into_boxed_slice(),
            mechanism_routes: mechanism_routes.into_boxed_slice(),
            requested_mechanisms,
            requested_index,
            rule_facet_dispositions,
        }
    }

    pub(crate) fn requested_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        self.requested_mechanisms.as_deref().unwrap_or_default()
    }

    pub(crate) fn shared_requested_mechanisms(&self) -> Option<Arc<[FamilyThemeMechanismKey]>> {
        self.requested_mechanisms.clone()
    }

    pub(crate) fn contains_requested_mechanism(&self, key: &FamilyThemeMechanismKey) -> bool {
        self.requested_index.contains(key)
    }

    pub(super) fn mechanism_key(&self, route: FamilyThemeRoute) -> FamilyThemeMechanismKey {
        mechanism_key(&self.spec, route)
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

    pub(super) fn ordinal_palette_disposition(
        &self,
        target: ThemeTarget,
    ) -> Option<FamilyThemeDisposition> {
        self.ordinal_palette_dispositions.get(&target).copied()
    }

    pub(super) fn effect_binding_indices(&self) -> &[usize] {
        &self.effect_binding_indices
    }

    pub(super) fn rule_facet_disposition(
        &self,
        rule_index: usize,
        facet: FamilyThemeRuleFacet,
    ) -> Option<FamilyThemeDisposition> {
        self.rule_facet_dispositions
            .get(&(rule_index, facet))
            .copied()
    }

    pub(super) fn has_mechanism_routes(&self) -> bool {
        !self.mechanism_routes.is_empty()
    }

    pub(super) fn mechanism_routes(&self) -> &[FamilyThemeRoute] {
        &self.mechanism_routes
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
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        self.resolve_style_internal(target, variant, ordinal, Some(work_meter))
    }

    fn resolve_style_internal(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        let winners = self.select_style(target, variant, ordinal, work_meter)?;
        Ok(self.materialize_style(winners))
    }

    fn select_style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ThemeStyleRuleWinners, OperationWorkError> {
        let mut winners = ThemeStyleRuleWinners::default();
        if let Some(slot) = self.slots.get(&target) {
            charge_ordinal_candidates(work_meter, ordinal, slot.ordinal_candidate_count(variant))?;
            slot.select_static(&mut winners, variant);
            slot.select_ordinal(&self.spec, &mut winners, variant, ordinal);
        }
        Ok(winners)
    }

    #[cfg(test)]
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
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        self.resolve_text_style_internal(target, variant, ordinal, Some(work_meter))
    }

    pub(super) fn resolve_text_style_with_variant_fallback(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        fallback: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        let mut winners = self.select_text_style(target, fallback, ordinal, Some(work_meter))?;
        if variant != fallback {
            // Preserve the second debit and checkpoint, even when it has no ordinal candidates.
            let variant_winners =
                self.select_text_style(target, variant, ordinal, Some(work_meter))?;
            winners.merge_from(&variant_winners);
        }
        Ok(self.materialize_style(winners))
    }

    pub(super) fn resolve_text_style_with_role_override(
        &self,
        fallback_target: ThemeTarget,
        override_target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        let mut winners =
            self.select_text_style(fallback_target, variant, ordinal, Some(work_meter))?;
        if self.slots.contains_key(&override_target) {
            let role = self.select_style(override_target, variant, ordinal, Some(work_meter))?;
            winners.overlay_role(&role);
        }
        Ok(self.materialize_style(winners))
    }

    fn resolve_text_style_internal(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ResolvedThemeStyle, OperationWorkError> {
        let winners = self.select_text_style(target, variant, ordinal, work_meter)?;
        Ok(self.materialize_style(winners))
    }

    fn select_text_style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<ThemeStyleRuleWinners, OperationWorkError> {
        let mut winners = ThemeStyleRuleWinners::default();
        let ordinal_candidate_count = self.ordinal_candidate_count(ThemeTarget::Text, variant)
            + if target == ThemeTarget::Text {
                0
            } else {
                self.ordinal_candidate_count(target, variant)
            };
        charge_ordinal_candidates(work_meter, ordinal, ordinal_candidate_count)?;
        self.select_text_target(ThemeTarget::Text, &mut winners, variant, ordinal);
        if target != ThemeTarget::Text {
            self.select_text_target(target, &mut winners, variant, ordinal);
        }
        Ok(winners)
    }

    fn materialize_style(&self, winners: ThemeStyleRuleWinners) -> ResolvedThemeStyle {
        ResolvedThemeStyle::from_rule_winners(
            Arc::clone(&self.spec),
            self.family,
            &self.base_typography,
            winners,
        )
    }

    fn ordinal_candidate_count(&self, target: ThemeTarget, variant: ThemeVariant) -> usize {
        self.slots
            .get(&target)
            .map_or(0, |slot| slot.ordinal_candidate_count(variant))
    }

    fn select_text_target(
        &self,
        target: ThemeTarget,
        winners: &mut ThemeStyleRuleWinners,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) {
        let Some(slot) = self.slots.get(&target) else {
            return;
        };
        slot.select_static(winners, variant);
        slot.select_ordinal(&self.spec, winners, variant, ordinal);
    }
}

impl FamilyRuleSlot {
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
                .or_default()
                .select_rule(patch, origin.rule_index()),
            (None, None) => self.common_static.select_rule(patch, origin.rule_index()),
        }
    }

    fn select_static(&self, winners: &mut ThemeStyleRuleWinners, variant: ThemeVariant) {
        winners.merge_from(&self.common_static);
        if let Some(variant_winners) = self.variant_static.get(&variant) {
            winners.merge_from(variant_winners);
        }
    }

    fn ordinal_candidate_count(&self, variant: ThemeVariant) -> usize {
        self.common_ordinal.len() + self.variant_ordinal.get(&variant).map_or(0, Vec::len)
    }

    fn select_ordinal(
        &self,
        spec: &DiagramThemeSpec,
        winners: &mut ThemeStyleRuleWinners,
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
                winners.select_rule(rules[origin.rule_index()].style(), origin.rule_index());
            }
        }
    }
}

fn mechanism_key(spec: &DiagramThemeSpec, route: FamilyThemeRoute) -> FamilyThemeMechanismKey {
    match route.mechanism() {
        FamilyThemeMechanism::BaseTypography(property) => {
            FamilyThemeMechanismKey::Typography(property)
        }
        FamilyThemeMechanism::RuleFacet {
            rule_index, target, ..
        } => FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        },
        FamilyThemeMechanism::OrdinalPalette { target } => {
            FamilyThemeMechanismKey::OrdinalPalette { target }
        }
        FamilyThemeMechanism::EffectBinding {
            binding_index,
            target,
        } => {
            let binding = &spec.effects().bindings()[binding_index];
            debug_assert_eq!(binding.target(), target);
            FamilyThemeMechanismKey::EffectBinding {
                target,
                effect_id: binding.effect_id().to_string(),
            }
        }
    }
}

fn charge_ordinal_candidates(
    work_meter: Option<&OperationWorkMeter>,
    ordinal: Option<usize>,
    candidates: usize,
) -> Result<(), OperationWorkError> {
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
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, OrdinalSelector, Specified, ThemeColorValue, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use merman_model::{CancelReason, OperationControl, OperationPhase};

    #[test]
    fn compound_text_queries_preserve_separate_ordered_ordinal_charges() {
        let mut rules = ThemeRuleSet::default();
        for (target, variant, ordinal) in [
            (ThemeTarget::Text, None, 1),
            (ThemeTarget::Title, Some(ThemeVariant::Default), 2),
            (ThemeTarget::Title, Some(ThemeVariant::Primary), 3),
            (ThemeTarget::Title, Some(ThemeVariant::Primary), 4),
            (ThemeTarget::ClusterLabel, None, 1),
            (ThemeTarget::ClusterLabel, None, 2),
            (ThemeTarget::ClusterLabel, Some(ThemeVariant::Default), 3),
        ] {
            let mut rule = ThemeRule::new(
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            )
            .with_ordinal(OrdinalSelector::exact(ordinal).unwrap());
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            rules = rules.with_rule(rule);
        }
        let program = FamilyThemeProgram::compile(
            Arc::new(DiagramThemeSpec::new().with_styles(rules)),
            DiagramFamilyId::FLOWCHART,
        );
        for role_override in [false, true] {
            // Neither branch matches ordinal 99, but their candidate debits are still 2 then 3.
            let resolve = |meter: &OperationWorkMeter| {
                if role_override {
                    program.resolve_text_style_with_role_override(
                        ThemeTarget::Title,
                        ThemeTarget::ClusterLabel,
                        ThemeVariant::Default,
                        Some(99),
                        meter,
                    )
                } else {
                    program.resolve_text_style_with_variant_fallback(
                        ThemeTarget::Title,
                        ThemeVariant::Primary,
                        ThemeVariant::Default,
                        Some(99),
                        meter,
                    )
                }
            };
            for limit in [1, 2, 4, 5] {
                let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap();
                let meter = OperationWorkMeter::new(policy);
                let result = resolve(&meter);
                if limit == 5 {
                    result.unwrap();
                    assert_eq!(meter.used(), 5);
                } else {
                    let attempted = if limit < 2 { 2 } else { 5 };
                    assert_eq!(
                        result.unwrap_err(),
                        OperationWorkError::ResourceLimitExceeded(
                            policy.check_layout_work_units(attempted).unwrap_err()
                        )
                    );
                    assert_eq!(meter.used(), if limit < 2 { 0 } else { 2 });
                }
            }
            let control = OperationControl::new();
            control.cancel_after_checkpoints(1);
            let meter = OperationWorkMeter::new_with_control(
                RenderResourcePolicy::unbounded_for_trusted_input(),
                control,
            );
            assert!(matches!(
                resolve(&meter),
                Err(OperationWorkError::Cancelled(error))
                    if error.phase == OperationPhase::Layout
                        && error.reason == CancelReason::Requested
            ));
            assert_eq!(meter.used(), 2);
        }

        for (target, variant, expected) in [
            (ThemeTarget::Title, ThemeVariant::Default, 2),
            (ThemeTarget::Text, ThemeVariant::Primary, 2),
        ] {
            let meter = OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, expected)
                    .unwrap(),
            );
            program
                .resolve_text_style_with_variant_fallback(
                    target,
                    variant,
                    ThemeVariant::Default,
                    Some(99),
                    &meter,
                )
                .unwrap();
            assert_eq!(meter.used(), expected);
        }
    }

    #[test]
    fn compound_text_queries_keep_zero_charge_checkpoints_and_role_slot_gate() {
        let program = FamilyThemeProgram::compile(
            Arc::new(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                        ))
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::ClusterLabel,
                                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                            )
                            .with_variant(ThemeVariant::Active),
                        ),
                ),
            ),
            DiagramFamilyId::FLOWCHART,
        );
        for (variant, ordinal, checkpoints) in [
            (ThemeVariant::Primary, Some(99), 2),
            (ThemeVariant::Default, Some(99), 1),
            (ThemeVariant::Primary, None, 0),
        ] {
            for allowed in 0..=checkpoints {
                let control = OperationControl::new();
                control.cancel_after_checkpoints(allowed);
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                    control.clone(),
                );
                let result = program.resolve_text_style_with_variant_fallback(
                    ThemeTarget::Title,
                    variant,
                    ThemeVariant::Default,
                    ordinal,
                    &meter,
                );
                if allowed < checkpoints {
                    assert!(matches!(
                        result,
                        Err(OperationWorkError::Cancelled(error))
                            if error.phase == OperationPhase::Layout
                    ));
                } else {
                    assert_eq!(result.unwrap().fill(), Some(&CanvasPaint::Transparent));
                    assert!(control.checkpoint_at(OperationPhase::Layout).is_err());
                }
                assert_eq!(meter.used(), 0);
            }
        }
        // An existing slot still checkpoints when its only rule has a different variant.
        for (target, ordinal, checkpoints) in [
            (ThemeTarget::ClusterLabel, Some(99), 2),
            (ThemeTarget::NodeLabel, Some(99), 1),
            (ThemeTarget::ClusterLabel, None, 0),
        ] {
            for allowed in 0..=checkpoints {
                let control = OperationControl::new();
                control.cancel_after_checkpoints(allowed);
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                    control.clone(),
                );
                let result = program.resolve_text_style_with_role_override(
                    ThemeTarget::Title,
                    target,
                    ThemeVariant::Default,
                    ordinal,
                    &meter,
                );
                if allowed < checkpoints {
                    assert!(matches!(
                        result,
                        Err(OperationWorkError::Cancelled(error))
                            if error.phase == OperationPhase::Layout
                    ));
                } else {
                    assert_eq!(result.unwrap().fill(), Some(&CanvasPaint::Transparent));
                    assert!(control.checkpoint_at(OperationPhase::Layout).is_err());
                }
                assert_eq!(meter.used(), 0);
            }
        }
    }

    #[test]
    fn cache_reuses_the_same_program_for_one_recipe_family() {
        let spec = Arc::new(
            DiagramThemeSpec::default().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        );
        let cache = FamilyThemeProgramCache::new(spec);

        let first = cache.get_or_compile(DiagramFamilyId::FLOWCHART);
        let second = cache.get_or_compile(DiagramFamilyId::FLOWCHART);
        let state = cache.get_or_compile(DiagramFamilyId::STATE);

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &state));
        assert!(Arc::ptr_eq(&first.spec, &cache.spec));
        assert!(Arc::ptr_eq(&state.spec, &cache.spec));
        let requested = first.shared_requested_mechanisms().unwrap();
        assert!(Arc::ptr_eq(
            &requested,
            &second.shared_requested_mechanisms().unwrap()
        ));
        assert!(state.requested_mechanisms().is_empty());
        assert!(state.shared_requested_mechanisms().is_none());
        drop(cache);
        drop(first);
        drop(second);
        assert_eq!(
            requested.as_ref(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn compiled_program_indexes_rule_facet_dispositions() {
        let paint = CanvasPaint::solid("#ef4444").expect("solid paint");
        let spec = Arc::new(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(paint.clone()),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        );
        let program = FamilyThemeProgram::compile(spec, DiagramFamilyId::FLOWCHART);
        let facet = FamilyThemeRuleFacet::fill(&Specified::Value(paint)).expect("fill facet");

        assert_eq!(
            program.rule_facet_disposition(0, facet),
            Some(FamilyThemeDisposition::TypedAdapter)
        );
    }

    #[test]
    fn native_evidence_requirements_preserve_order_and_mixed_facet_ownership() {
        let fill = CanvasPaint::solid("#123456").unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(fill.clone()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(fill.clone())
                                .with_stroke(fill.clone()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(fill.clone())
                                .with_stroke(fill),
                        )),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let program = theme.family_program_owner();
        let expected = vec![
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            },
            FamilyThemeMechanismKey::Rule {
                index: 1,
                target: ThemeTarget::Node,
            },
            FamilyThemeMechanismKey::Rule {
                index: 2,
                target: ThemeTarget::Text,
            },
        ];
        // Shadowed rules retain identity even when Class owns only some of their facets.
        assert_eq!(program.requested_mechanisms(), expected.as_slice());
        for key in &expected {
            assert!(program.contains_requested_mechanism(key));
        }
        assert!(
            !program.contains_requested_mechanism(&FamilyThemeMechanismKey::Rule {
                index: 3,
                target: ThemeTarget::Text,
            })
        );
    }

    #[test]
    fn native_evidence_requirements_cover_empty_typed_text_and_many_rules() {
        let empty = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let fill = CanvasPaint::solid("#123456").unwrap();
        let typed_text = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill.clone()),
                    ),
                )),
            )
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let empty_program = empty.family_program_owner();
        assert!(empty_program.requested_mechanisms().is_empty());
        assert!(empty_program.shared_requested_mechanisms().is_none());
        assert!(
            !empty_program.contains_requested_mechanism(&FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            })
        );
        let typed_program = typed_text.family_program_owner();
        assert_eq!(
            typed_program.requested_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            }]
        );

        let excluded = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Canvas,
                            ThemeStylePatch::default().with_fill(fill.clone()),
                        ))
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default().with_fill(fill.clone()),
                            )
                            .for_family(DiagramFamilyId::FLOWCHART),
                        ),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let excluded_program = excluded.family_program_owner();
        assert!(excluded_program.rule_indices().is_empty());
        assert!(excluded_program.mechanism_routes().is_empty());
        assert!(excluded_program.shared_requested_mechanisms().is_none());

        let mut rules = ThemeRuleSet::default();
        for _ in 0..512 {
            rules = rules.with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default()
                    .with_fill(fill.clone())
                    .with_stroke(fill.clone()),
            ));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let program = theme.family_program_owner();
        assert_eq!(theme.family_mechanism_routes().len(), 1024);
        let expected = (0..512)
            .map(|index| FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Node,
            })
            .collect::<Vec<_>>();
        assert_eq!(program.requested_mechanisms(), expected.as_slice());
    }

    #[test]
    fn native_evidence_effect_requirements_keep_binding_order_and_full_identity() {
        use crate::diagram_theme::{
            DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
        };

        let mut effects = DiagramEffectSet::default();
        for id in ["z-last", "a-first"] {
            effects = effects
                .with_graph(
                    EffectGraph::new(
                        id,
                        [EffectPrimitive::DropShadow {
                            input: EffectInput::SourceGraphic,
                            offset_x: 1.0,
                            offset_y: 1.0,
                            blur_radius: 2.0,
                            spread: 0.0,
                            color: ThemeColorValue::parse("#00000080").unwrap(),
                        }],
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let bindings = [
            (ThemeTarget::Edge, "z-last"),
            (ThemeTarget::Node, "a-first"),
            (ThemeTarget::Cluster, "z-last"),
        ];
        for (target, id) in bindings {
            effects = effects
                .with_binding(EffectBinding::new(target, id).unwrap())
                .unwrap();
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects))
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let program = theme.family_program_owner();
        let expected = bindings
            .into_iter()
            .map(|(target, id)| FamilyThemeMechanismKey::EffectBinding {
                target,
                effect_id: id.to_string(),
            })
            .collect::<Vec<_>>();
        assert_eq!(program.requested_mechanisms(), expected.as_slice());
    }
}
