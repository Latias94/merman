use crate::DiagramFamilyId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::canvas::{CanvasPaint, InsetsPx, ThemeColorValue};
use super::family_program::FamilyThemeProgram;
use super::semantic::{
    OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeRule, ThemeStylePatch, ThemeTarget,
    ThemeVariant,
};
use super::typography::{Specified, TextStyle, TextStylePatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThemeRuleOrigin {
    rule_index: usize,
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    ordinal: Option<OrdinalSelector>,
}

impl ThemeRuleOrigin {
    pub(super) fn new(rule_index: usize, rule: &ThemeRule) -> Self {
        Self {
            rule_index,
            target: rule.target(),
            variant: rule.variant(),
            ordinal: rule.ordinal(),
        }
    }

    pub(crate) const fn rule_index(self) -> usize {
        self.rule_index
    }

    #[cfg(test)]
    pub(crate) const fn target(self) -> ThemeTarget {
        self.target
    }

    pub(crate) const fn variant(self) -> Option<ThemeVariant> {
        self.variant
    }

    pub(crate) const fn ordinal(self) -> Option<OrdinalSelector> {
        self.ordinal
    }

    fn supersedes(self, current: Option<Self>) -> bool {
        current.is_none_or(|current| self.rule_index >= current.rule_index)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedProperty<T> {
    specified: Specified<T>,
    winner: Option<ThemeRuleOrigin>,
}

impl<T: PartialEq> PartialEq for ResolvedProperty<T> {
    fn eq(&self, other: &Self) -> bool {
        self.specified == other.specified
    }
}

impl<T> Default for ResolvedProperty<T> {
    fn default() -> Self {
        Self {
            specified: Specified::Unspecified,
            winner: None,
        }
    }
}

impl<T> ResolvedProperty<T> {
    fn apply(&mut self, specified: &Specified<T>, origin: ThemeRuleOrigin)
    where
        T: Clone,
    {
        if specified.is_unspecified() || !origin.supersedes(self.winner) {
            return;
        }
        self.specified = specified.clone();
        self.winner = Some(origin);
    }

    fn merge_from(&mut self, other: &Self)
    where
        T: Clone,
    {
        if let Some(origin) = other.winner {
            self.apply(&other.specified, origin);
        }
    }

    pub const fn specified(&self) -> &Specified<T> {
        &self.specified
    }

    pub(crate) const fn winner(&self) -> Option<ThemeRuleOrigin> {
        self.winner
    }

    pub const fn value(&self) -> Option<&T> {
        match &self.specified {
            Specified::Value(value) => Some(value),
            Specified::Unspecified | Specified::Clear => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub(crate) enum ThemeTypographyProperty {
    FontStack,
    FontSize,
    FontWeight,
    FontStyle,
    LineHeight,
    LetterSpacing,
    WordSpacing,
    Transform,
    Decoration,
    TextAlign,
    WhiteSpace,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ResolvedStyleProperty {
    Fill,
    Stroke,
    StrokeWidth,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
    Opacity,
    FillOpacity,
    StrokeOpacity,
    Radius,
    Padding,
    Typography(ThemeTypographyProperty),
    Effect,
}

#[derive(Debug, Clone)]
pub struct ResolvedTypography {
    base: TextStyle,
    computed: TextStyle,
    patch: TextStylePatch,
    winners: BTreeMap<ThemeTypographyProperty, ThemeRuleOrigin>,
}

impl PartialEq for ResolvedTypography {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.computed == other.computed && self.patch == other.patch
    }
}

impl ResolvedTypography {
    pub(super) fn new(base: TextStyle) -> Self {
        Self {
            computed: base.clone(),
            base,
            patch: TextStylePatch::default(),
            winners: BTreeMap::new(),
        }
    }

    fn apply(&mut self, patch: &TextStylePatch, origin: ThemeRuleOrigin) {
        apply_typography_property(
            &patch.font_stack,
            &mut self.patch.font_stack,
            ThemeTypographyProperty::FontStack,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.font_size_px,
            &mut self.patch.font_size_px,
            ThemeTypographyProperty::FontSize,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.font_weight,
            &mut self.patch.font_weight,
            ThemeTypographyProperty::FontWeight,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.font_style,
            &mut self.patch.font_style,
            ThemeTypographyProperty::FontStyle,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.line_height,
            &mut self.patch.line_height,
            ThemeTypographyProperty::LineHeight,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.letter_spacing_px,
            &mut self.patch.letter_spacing_px,
            ThemeTypographyProperty::LetterSpacing,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.word_spacing_px,
            &mut self.patch.word_spacing_px,
            ThemeTypographyProperty::WordSpacing,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.transform,
            &mut self.patch.transform,
            ThemeTypographyProperty::Transform,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.decoration,
            &mut self.patch.decoration,
            ThemeTypographyProperty::Decoration,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.text_align,
            &mut self.patch.text_align,
            ThemeTypographyProperty::TextAlign,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.white_space,
            &mut self.patch.white_space,
            ThemeTypographyProperty::WhiteSpace,
            origin,
            &mut self.winners,
        );
        apply_typography_property(
            &patch.wrap,
            &mut self.patch.wrap,
            ThemeTypographyProperty::Wrap,
            origin,
            &mut self.winners,
        );
        self.computed = self.base.clone();
        self.patch.apply_to(&mut self.computed, &self.base);
    }

    fn merge_from(&mut self, other: &Self) {
        merge_typography_property(
            &other.patch.font_stack,
            ThemeTypographyProperty::FontStack,
            other,
            &mut self.patch.font_stack,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.font_size_px,
            ThemeTypographyProperty::FontSize,
            other,
            &mut self.patch.font_size_px,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.font_weight,
            ThemeTypographyProperty::FontWeight,
            other,
            &mut self.patch.font_weight,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.font_style,
            ThemeTypographyProperty::FontStyle,
            other,
            &mut self.patch.font_style,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.line_height,
            ThemeTypographyProperty::LineHeight,
            other,
            &mut self.patch.line_height,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.letter_spacing_px,
            ThemeTypographyProperty::LetterSpacing,
            other,
            &mut self.patch.letter_spacing_px,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.word_spacing_px,
            ThemeTypographyProperty::WordSpacing,
            other,
            &mut self.patch.word_spacing_px,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.transform,
            ThemeTypographyProperty::Transform,
            other,
            &mut self.patch.transform,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.decoration,
            ThemeTypographyProperty::Decoration,
            other,
            &mut self.patch.decoration,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.text_align,
            ThemeTypographyProperty::TextAlign,
            other,
            &mut self.patch.text_align,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.white_space,
            ThemeTypographyProperty::WhiteSpace,
            other,
            &mut self.patch.white_space,
            &mut self.winners,
        );
        merge_typography_property(
            &other.patch.wrap,
            ThemeTypographyProperty::Wrap,
            other,
            &mut self.patch.wrap,
            &mut self.winners,
        );
        self.computed = self.base.clone();
        self.patch.apply_to(&mut self.computed, &self.base);
    }

    pub const fn base(&self) -> &TextStyle {
        &self.base
    }

    pub const fn computed(&self) -> &TextStyle {
        &self.computed
    }

    pub const fn patch(&self) -> &TextStylePatch {
        &self.patch
    }

    pub(crate) fn winner(&self, property: ThemeTypographyProperty) -> Option<ThemeRuleOrigin> {
        self.winners.get(&property).copied()
    }
}

fn apply_typography_property<T: Clone>(
    incoming: &Specified<T>,
    resolved: &mut Specified<T>,
    property: ThemeTypographyProperty,
    origin: ThemeRuleOrigin,
    winners: &mut BTreeMap<ThemeTypographyProperty, ThemeRuleOrigin>,
) {
    if incoming.is_unspecified() || !origin.supersedes(winners.get(&property).copied()) {
        return;
    }
    *resolved = incoming.clone();
    winners.insert(property, origin);
}

fn merge_typography_property<T: Clone>(
    incoming: &Specified<T>,
    property: ThemeTypographyProperty,
    incoming_style: &ResolvedTypography,
    resolved: &mut Specified<T>,
    winners: &mut BTreeMap<ThemeTypographyProperty, ThemeRuleOrigin>,
) {
    if let Some(origin) = incoming_style.winner(property) {
        apply_typography_property(incoming, resolved, property, origin, winners);
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedThemeStyle {
    fill: ResolvedProperty<CanvasPaint>,
    stroke: ResolvedProperty<CanvasPaint>,
    stroke_width: ResolvedProperty<f32>,
    stroke_dasharray: ResolvedProperty<Vec<f32>>,
    stroke_linecap: ResolvedProperty<StrokeLineCap>,
    stroke_linejoin: ResolvedProperty<StrokeLineJoin>,
    opacity: ResolvedProperty<f32>,
    fill_opacity: ResolvedProperty<f32>,
    stroke_opacity: ResolvedProperty<f32>,
    radius: ResolvedProperty<f32>,
    padding: ResolvedProperty<InsetsPx>,
    typography: ResolvedTypography,
    effect: ResolvedProperty<String>,
    matched_rule_indices: BTreeSet<usize>,
}

impl PartialEq for ResolvedThemeStyle {
    fn eq(&self, other: &Self) -> bool {
        self.fill == other.fill
            && self.stroke == other.stroke
            && self.stroke_width == other.stroke_width
            && self.stroke_dasharray == other.stroke_dasharray
            && self.stroke_linecap == other.stroke_linecap
            && self.stroke_linejoin == other.stroke_linejoin
            && self.opacity == other.opacity
            && self.fill_opacity == other.fill_opacity
            && self.stroke_opacity == other.stroke_opacity
            && self.radius == other.radius
            && self.padding == other.padding
            && self.typography == other.typography
            && self.effect == other.effect
    }
}

impl ResolvedThemeStyle {
    pub(super) fn new(typography: TextStyle) -> Self {
        Self {
            fill: ResolvedProperty::default(),
            stroke: ResolvedProperty::default(),
            stroke_width: ResolvedProperty::default(),
            stroke_dasharray: ResolvedProperty::default(),
            stroke_linecap: ResolvedProperty::default(),
            stroke_linejoin: ResolvedProperty::default(),
            opacity: ResolvedProperty::default(),
            fill_opacity: ResolvedProperty::default(),
            stroke_opacity: ResolvedProperty::default(),
            radius: ResolvedProperty::default(),
            padding: ResolvedProperty::default(),
            typography: ResolvedTypography::new(typography),
            effect: ResolvedProperty::default(),
            matched_rule_indices: BTreeSet::new(),
        }
    }

    pub(super) fn apply(&mut self, patch: &ThemeStylePatch, origin: ThemeRuleOrigin) {
        if !patch.is_empty() {
            self.matched_rule_indices.insert(origin.rule_index());
        }
        self.fill.apply(&patch.paint.fill, origin);
        self.stroke.apply(&patch.stroke.paint, origin);
        self.stroke_width.apply(&patch.stroke.width, origin);
        self.stroke_dasharray.apply(&patch.stroke.dasharray, origin);
        self.stroke_linecap.apply(&patch.stroke.linecap, origin);
        self.stroke_linejoin.apply(&patch.stroke.linejoin, origin);
        self.opacity.apply(&patch.paint.opacity, origin);
        self.fill_opacity.apply(&patch.paint.fill_opacity, origin);
        self.stroke_opacity
            .apply(&patch.stroke.stroke_opacity, origin);
        self.radius.apply(&patch.geometry.radius, origin);
        self.padding.apply(&patch.spacing.padding, origin);
        self.typography.apply(&patch.typography, origin);
        self.effect.apply(&patch.effects.effect, origin);
    }

    pub(super) fn merge_from(&mut self, other: &Self) {
        self.fill.merge_from(&other.fill);
        self.stroke.merge_from(&other.stroke);
        self.stroke_width.merge_from(&other.stroke_width);
        self.stroke_dasharray.merge_from(&other.stroke_dasharray);
        self.stroke_linecap.merge_from(&other.stroke_linecap);
        self.stroke_linejoin.merge_from(&other.stroke_linejoin);
        self.opacity.merge_from(&other.opacity);
        self.fill_opacity.merge_from(&other.fill_opacity);
        self.stroke_opacity.merge_from(&other.stroke_opacity);
        self.radius.merge_from(&other.radius);
        self.padding.merge_from(&other.padding);
        self.typography.merge_from(&other.typography);
        self.effect.merge_from(&other.effect);
        self.matched_rule_indices
            .extend(other.matched_rule_indices.iter().copied());
    }

    pub const fn fill(&self) -> Option<&CanvasPaint> {
        self.fill.value()
    }

    pub(crate) fn winner_rule_properties(&self) -> Vec<(ResolvedStyleProperty, ThemeRuleOrigin)> {
        let mut winners = Vec::new();
        macro_rules! collect_property_winner {
            ($kind:expr, $property:expr) => {
                if let Some(origin) = $property.winner() {
                    winners.push(($kind, origin));
                }
            };
        }
        collect_property_winner!(ResolvedStyleProperty::Fill, self.fill);
        collect_property_winner!(ResolvedStyleProperty::Stroke, self.stroke);
        collect_property_winner!(ResolvedStyleProperty::StrokeWidth, self.stroke_width);
        collect_property_winner!(
            ResolvedStyleProperty::StrokeDasharray,
            self.stroke_dasharray
        );
        collect_property_winner!(ResolvedStyleProperty::StrokeLinecap, self.stroke_linecap);
        collect_property_winner!(ResolvedStyleProperty::StrokeLinejoin, self.stroke_linejoin);
        collect_property_winner!(ResolvedStyleProperty::Opacity, self.opacity);
        collect_property_winner!(ResolvedStyleProperty::FillOpacity, self.fill_opacity);
        collect_property_winner!(ResolvedStyleProperty::StrokeOpacity, self.stroke_opacity);
        collect_property_winner!(ResolvedStyleProperty::Radius, self.radius);
        collect_property_winner!(ResolvedStyleProperty::Padding, self.padding);
        collect_property_winner!(ResolvedStyleProperty::Effect, self.effect);
        for property in [
            ThemeTypographyProperty::FontStack,
            ThemeTypographyProperty::FontSize,
            ThemeTypographyProperty::FontWeight,
            ThemeTypographyProperty::FontStyle,
            ThemeTypographyProperty::LineHeight,
            ThemeTypographyProperty::LetterSpacing,
            ThemeTypographyProperty::WordSpacing,
            ThemeTypographyProperty::Transform,
            ThemeTypographyProperty::Decoration,
            ThemeTypographyProperty::TextAlign,
            ThemeTypographyProperty::WhiteSpace,
            ThemeTypographyProperty::Wrap,
        ] {
            if let Some(origin) = self.typography.winner(property) {
                winners.push((ResolvedStyleProperty::Typography(property), origin));
            }
        }
        winners
    }

    pub(crate) fn matched_rule_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.matched_rule_indices.iter().copied()
    }

    pub const fn stroke(&self) -> Option<&CanvasPaint> {
        self.stroke.value()
    }

    pub const fn stroke_width(&self) -> Option<f32> {
        match self.stroke_width.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub fn stroke_dasharray(&self) -> Option<&[f32]> {
        self.stroke_dasharray.value().map(Vec::as_slice)
    }

    pub const fn stroke_linecap(&self) -> Option<StrokeLineCap> {
        match self.stroke_linecap.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn stroke_linejoin(&self) -> Option<StrokeLineJoin> {
        match self.stroke_linejoin.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn opacity(&self) -> Option<f32> {
        match self.opacity.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn fill_opacity(&self) -> Option<f32> {
        match self.fill_opacity.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn stroke_opacity(&self) -> Option<f32> {
        match self.stroke_opacity.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn radius(&self) -> Option<f32> {
        match self.radius.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn padding(&self) -> Option<InsetsPx> {
        match self.padding.value() {
            Some(value) => Some(*value),
            None => None,
        }
    }

    pub const fn typography(&self) -> &TextStyle {
        self.typography.computed()
    }

    pub fn effect(&self) -> Option<&str> {
        self.effect.value().map(String::as_str)
    }

    pub const fn fill_resolution(&self) -> &ResolvedProperty<CanvasPaint> {
        &self.fill
    }

    pub const fn stroke_resolution(&self) -> &ResolvedProperty<CanvasPaint> {
        &self.stroke
    }

    pub const fn stroke_width_resolution(&self) -> &ResolvedProperty<f32> {
        &self.stroke_width
    }

    pub const fn stroke_dasharray_resolution(&self) -> &ResolvedProperty<Vec<f32>> {
        &self.stroke_dasharray
    }

    pub const fn stroke_linecap_resolution(&self) -> &ResolvedProperty<StrokeLineCap> {
        &self.stroke_linecap
    }

    pub const fn stroke_linejoin_resolution(&self) -> &ResolvedProperty<StrokeLineJoin> {
        &self.stroke_linejoin
    }

    pub const fn opacity_resolution(&self) -> &ResolvedProperty<f32> {
        &self.opacity
    }

    pub const fn fill_opacity_resolution(&self) -> &ResolvedProperty<f32> {
        &self.fill_opacity
    }

    pub const fn stroke_opacity_resolution(&self) -> &ResolvedProperty<f32> {
        &self.stroke_opacity
    }

    pub const fn radius_resolution(&self) -> &ResolvedProperty<f32> {
        &self.radius
    }

    pub const fn padding_resolution(&self) -> &ResolvedProperty<InsetsPx> {
        &self.padding
    }

    pub const fn typography_resolution(&self) -> &ResolvedTypography {
        &self.typography
    }

    pub const fn effect_resolution(&self) -> &ResolvedProperty<String> {
        &self.effect
    }
}

/// Immutable family projection of a compiled diagram theme. It carries no mutable renderer state.
#[derive(Debug, Clone)]
pub struct ResolvedDiagramTheme {
    theme: super::DiagramTheme,
    family: DiagramFamilyId,
    program: Arc<FamilyThemeProgram>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResolvedThemeEffect<'a> {
    ClearedByRule,
    Rule {
        graph: Option<&'a super::effects::EffectGraph>,
    },
    Binding {
        binding: &'a super::effects::EffectBinding,
        graph: Option<&'a super::effects::EffectGraph>,
    },
}

impl ResolvedDiagramTheme {
    pub(crate) fn new(theme: super::DiagramTheme, family: DiagramFamilyId) -> Self {
        let program = theme.family_program(family);
        Self {
            theme,
            family,
            program,
        }
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family
    }

    pub fn typography(&self) -> &TextStyle {
        self.program.base_typography()
    }

    #[cfg(test)]
    pub(crate) fn has_family_style_requirements(&self) -> bool {
        self.program.has_family_style_requirements()
    }

    pub(crate) fn effect_bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &super::effects::EffectBinding> + '_ {
        self.program
            .effect_binding_indices()
            .iter()
            .map(|index| &self.theme.spec().effects().bindings()[*index])
    }

    pub(crate) fn resolve_effect<'a>(
        &'a self,
        target: ThemeTarget,
        resolution: &ResolvedProperty<String>,
    ) -> Option<ResolvedThemeEffect<'a>> {
        match resolution.specified() {
            Specified::Clear => Some(ResolvedThemeEffect::ClearedByRule),
            Specified::Value(effect_id) => Some(ResolvedThemeEffect::Rule {
                graph: self.theme.spec().effects().graph(effect_id),
            }),
            Specified::Unspecified => self
                .effect_bindings()
                .find(|binding| binding.target() == target)
                .map(|binding| ResolvedThemeEffect::Binding {
                    graph: self.theme.spec().effects().graph(binding.effect_id()),
                    binding,
                }),
        }
    }

    pub(crate) fn effect_graph(&self, effect_id: &str) -> Option<&super::effects::EffectGraph> {
        self.theme.spec().effects().graph(effect_id)
    }

    pub(crate) fn family_rules(&self) -> impl Iterator<Item = (usize, &ThemeRule)> + '_ {
        self.program
            .rule_indices()
            .iter()
            .copied()
            .map(|index| (index, &self.theme.spec().styles().rules()[index]))
    }

    pub(crate) fn family_ordinal_palette_targets(&self) -> impl Iterator<Item = ThemeTarget> + '_ {
        self.program.ordinal_palette_targets()
    }

    pub(crate) fn ordinal_palette_disposition(
        &self,
        target: ThemeTarget,
    ) -> Option<super::family_mechanism_matrix::FamilyThemeDisposition> {
        self.program.ordinal_palette_disposition(target)
    }

    pub(crate) fn family_mechanism_keys(&self) -> Vec<super::application::FamilyThemeMechanismKey> {
        let mut keys = Vec::new();
        for route in self.program.mechanism_routes().iter().copied() {
            let key = self.family_mechanism_key(route);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys
    }

    pub(crate) fn family_mechanism_key(
        &self,
        route: super::family_mechanism_matrix::FamilyThemeRoute,
    ) -> super::application::FamilyThemeMechanismKey {
        use super::application::FamilyThemeMechanismKey;
        use super::family_mechanism_matrix::FamilyThemeMechanism;

        match route.mechanism() {
            FamilyThemeMechanism::BaseTypography(_) => FamilyThemeMechanismKey::Typography,
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
                let binding = &self.theme.spec().effects().bindings()[binding_index];
                debug_assert_eq!(binding.target(), target);
                FamilyThemeMechanismKey::EffectBinding {
                    target,
                    effect_id: binding.effect_id().to_string(),
                }
            }
        }
    }

    pub(crate) fn family_mechanism_routes(
        &self,
    ) -> &[super::family_mechanism_matrix::FamilyThemeRoute] {
        self.program.mechanism_routes()
    }

    pub(crate) fn has_family_mechanism_routes(&self) -> bool {
        self.program.has_mechanism_routes()
    }

    pub(crate) fn rule_facet_disposition(
        &self,
        rule_index: usize,
        facet: super::family_mechanism_matrix::FamilyThemeRuleFacet,
    ) -> Option<super::family_mechanism_matrix::FamilyThemeDisposition> {
        self.program.rule_facet_disposition(rule_index, facet)
    }

    pub fn style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        self.program.resolve_style(target, variant, ordinal)
    }

    pub(crate) fn style_with_work_meter(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, crate::resources::OperationWorkError> {
        self.program
            .resolve_style_with_work_meter(target, variant, ordinal, work_meter)
    }

    #[cfg(test)]
    pub(crate) fn text_style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        self.program.resolve_text_style(target, variant, ordinal)
    }

    pub(crate) fn text_style_with_work_meter(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, crate::resources::OperationWorkError> {
        self.program
            .resolve_text_style_with_work_meter(target, variant, ordinal, work_meter)
    }

    pub fn series_color(
        &self,
        target: ThemeTarget,
        one_based_index: usize,
    ) -> Option<&ThemeColorValue> {
        let index = self.program.ordinal_palette_index(target)?;
        self.theme.spec().styles().ordinal_palettes()[index]
            .1
            .color_for(one_based_index)
    }
}

/// Independent rule-order oracle used to verify the compiled family program in tests.
#[cfg(test)]
pub(crate) fn resolve_style(
    spec: &super::DiagramThemeSpec,
    family: DiagramFamilyId,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
) -> ResolvedThemeStyle {
    let base_typography = spec.typography().family_style(family).clone();
    resolve_matching_rules(
        &base_typography,
        spec.styles()
            .rules()
            .iter()
            .enumerate()
            .filter(|(_, rule)| {
                target.valid_for(family)
                    && rule.target() == target
                    && rule.applies_to(family, variant, ordinal)
            }),
    )
}

#[cfg(test)]
fn resolve_matching_rules<'a>(
    base_typography: &TextStyle,
    rules: impl Iterator<Item = (usize, &'a ThemeRule)>,
) -> ResolvedThemeStyle {
    let mut style = ResolvedThemeStyle::new(base_typography.clone());
    for (rule_index, rule) in rules {
        style.apply(rule.style(), ThemeRuleOrigin::new(rule_index, rule));
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasSpec, DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding,
        EffectGraph, EffectInput, EffectPrimitive, OrdinalPalette, OrdinalSelector, ThemeRule,
        ThemeRuleSet,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn family_resolution_indexes_only_applicable_rules_and_palettes() {
        let node_fill = CanvasPaint::solid("#ef4444").expect("valid fill");
        let node_stroke = CanvasPaint::solid("#2563eb").expect("valid stroke");
        let state_fill = CanvasPaint::solid("#22c55e").expect("valid state fill");
        let state_text_fill = CanvasPaint::solid("#a855f7").expect("valid text fill");
        let palette_a = ThemeColorValue::parse("#f8fafc").expect("valid palette color");
        let palette_b = ThemeColorValue::parse("#94a3b8").expect("valid palette color");
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(node_fill.clone()),
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_stroke(node_stroke.clone()),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            )
            .with_rule(ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default().with_fill(state_fill),
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(state_text_fill),
                )
                .for_family(DiagramFamilyId::STATE),
            )
            .with_ordinal_palette(
                ThemeTarget::ChartSeries,
                OrdinalPalette::new([ThemeColorValue::parse("#0f172a").unwrap()]).unwrap(),
            )
            .with_ordinal_palette(
                ThemeTarget::Node,
                OrdinalPalette::new([palette_a.clone(), palette_b.clone()]).unwrap(),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);

        let node = resolved.style(ThemeTarget::Node, ThemeVariant::Default, None);
        assert_eq!(node.fill(), Some(&node_fill));
        assert_eq!(node.stroke(), Some(&node_stroke));
        assert_eq!(
            resolved
                .style(ThemeTarget::State, ThemeVariant::Default, None)
                .fill(),
            None
        );
        assert_eq!(
            resolved
                .style(ThemeTarget::Text, ThemeVariant::Default, None)
                .fill(),
            None
        );
        assert_eq!(resolved.series_color(ThemeTarget::Node, 0), None);
        assert_eq!(
            resolved.series_color(ThemeTarget::Node, 1),
            Some(&palette_a)
        );
        assert_eq!(
            resolved.series_color(ThemeTarget::Node, 2),
            Some(&palette_b)
        );
        assert_eq!(
            resolved.series_color(ThemeTarget::Node, 3),
            Some(&palette_a)
        );
        assert_eq!(resolved.series_color(ThemeTarget::Edge, 1), None);
        assert_eq!(resolved.series_color(ThemeTarget::ChartSeries, 1), None);
    }

    #[test]
    fn family_resolution_excludes_root_canvas_and_indexes_local_effect_bindings() {
        let state_fill = CanvasPaint::solid("#22c55e").expect("valid state fill");
        let graph = EffectGraph::new(
            "state-shadow",
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#00000080").expect("valid shadow color"),
            }],
        )
        .expect("valid effect graph");
        let effects = DiagramEffectSet::default()
            .with_graph(graph)
            .expect("add effect graph")
            .with_binding(
                EffectBinding::new(ThemeTarget::State, "state-shadow")
                    .expect("valid state effect binding"),
            )
            .expect("add state effect binding");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_canvas(CanvasSpec::solid("#0f172a").expect("valid canvas"))
                    .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default().with_fill(state_fill),
                    )))
                    .with_effects(effects),
            )
            .expect("compile theme");

        let resolved = theme.resolve(DiagramFamilyId::STATE);
        assert_eq!(
            resolved
                .style(ThemeTarget::Canvas, ThemeVariant::Default, None)
                .fill(),
            None
        );
        assert_eq!(resolved.effect_bindings().count(), 1);
        assert!(resolved.has_family_style_requirements());
    }

    #[test]
    fn indexed_style_resolution_matches_the_compiled_spec_order() {
        let base_fill = CanvasPaint::solid("#ef4444").expect("valid base fill");
        let variant_stroke = CanvasPaint::solid("#2563eb").expect("valid variant stroke");
        let ordinal_fill = CanvasPaint::solid("#22c55e").expect("valid ordinal fill");
        let base = ThemeStylePatch::default().with_fill(base_fill.clone());
        let variant = ThemeStylePatch::default().with_stroke(variant_stroke.clone());
        let ordinal = ThemeStylePatch::default().with_fill(ordinal_fill.clone());
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, base))
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, variant).with_variant(ThemeVariant::Active),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, ordinal)
                    .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);

        for variant in ThemeVariant::ALL.iter().copied() {
            for ordinal in [None, Some(0), Some(1), Some(2), Some(3)] {
                assert_eq!(
                    resolved.style(ThemeTarget::Node, variant, ordinal),
                    resolve_style(
                        theme.spec(),
                        DiagramFamilyId::FLOWCHART,
                        ThemeTarget::Node,
                        variant,
                        ordinal,
                    )
                );
            }
        }

        let default = resolved.style(ThemeTarget::Node, ThemeVariant::Default, None);
        assert_eq!(default.fill(), Some(&base_fill));
        assert_eq!(default.stroke(), None);

        let active = resolved.style(ThemeTarget::Node, ThemeVariant::Active, None);
        assert_eq!(active.fill(), Some(&base_fill));
        assert_eq!(active.stroke(), Some(&variant_stroke));

        let default_second = resolved.style(ThemeTarget::Node, ThemeVariant::Default, Some(2));
        assert_eq!(default_second.fill(), Some(&ordinal_fill));
        assert_eq!(default_second.stroke(), None);

        let active_first = resolved.style(ThemeTarget::Node, ThemeVariant::Active, Some(1));
        assert_eq!(active_first.fill(), Some(&base_fill));
        assert_eq!(active_first.stroke(), Some(&variant_stroke));

        let active_second = resolved.style(ThemeTarget::Node, ThemeVariant::Active, Some(2));
        assert_eq!(active_second.fill(), Some(&ordinal_fill));
        assert_eq!(active_second.stroke(), Some(&variant_stroke));
    }

    #[test]
    fn resolved_style_preserves_unspecified_clear_and_rule_origin() {
        let fill = CanvasPaint::solid("#ef4444").expect("valid fill");
        let mut base = ThemeStylePatch::default().with_fill(fill.clone());
        base.typography.font_size_px = Specified::Value(22.0);
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        clear.typography.font_size_px = Specified::Clear;
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, base))
            .with_rule(ThemeRule::new(ThemeTarget::Node, clear).with_variant(ThemeVariant::Active));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile theme");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);

        let default = resolved.style(ThemeTarget::Node, ThemeVariant::Default, None);
        assert_eq!(default.fill(), Some(&fill));
        assert!(matches!(
            default.fill_resolution().specified(),
            Specified::Value(value) if value == &fill
        ));
        assert_eq!(default.fill_resolution().winner().unwrap().rule_index(), 0);
        assert_eq!(default.typography().font_size_px(), 22.0);
        assert!(matches!(
            default.typography_resolution().patch().font_size_px,
            Specified::Value(22.0)
        ));

        let active = resolved.style(ThemeTarget::Node, ThemeVariant::Active, None);
        assert_eq!(active.fill(), None);
        assert!(matches!(
            active.fill_resolution().specified(),
            Specified::Clear
        ));
        assert_eq!(active.fill_resolution().winner().unwrap().rule_index(), 1);
        assert_eq!(active.typography().font_size_px(), 16.0);
        assert!(matches!(
            active.typography_resolution().patch().font_size_px,
            Specified::Clear
        ));
        assert_eq!(
            active
                .typography_resolution()
                .winner(ThemeTypographyProperty::FontSize)
                .unwrap()
                .rule_index(),
            1
        );

        assert!(matches!(
            active.stroke_resolution().specified(),
            Specified::Unspecified
        ));
        assert_eq!(active.stroke_resolution().winner(), None);
    }

    #[test]
    fn compiled_program_preserves_static_and_ordinal_source_order_per_property() {
        let ordinal_fill = CanvasPaint::solid("#ef4444").expect("valid ordinal fill");
        let late_fill = CanvasPaint::solid("#2563eb").expect("valid late fill");
        let mut ordinal_patch = ThemeStylePatch::default().with_fill(ordinal_fill);
        ordinal_patch.typography.font_size_px = Specified::Value(24.0);
        let mut late_static_patch = ThemeStylePatch::default().with_fill(late_fill.clone());
        late_static_patch.typography.font_size_px = Specified::Clear;
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, ordinal_patch)
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(ThemeRule::new(ThemeTarget::Node, late_static_patch));
        let resolved = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile interleaved theme")
            .resolve(DiagramFamilyId::FLOWCHART)
            .style(ThemeTarget::Node, ThemeVariant::Default, Some(1));

        assert_eq!(resolved.fill(), Some(&late_fill));
        assert_eq!(resolved.fill_resolution().winner().unwrap().rule_index(), 1);
        assert!(matches!(
            resolved.typography_resolution().patch().font_size_px,
            Specified::Clear
        ));
        assert_eq!(
            resolved
                .typography_resolution()
                .winner(ThemeTypographyProperty::FontSize)
                .unwrap()
                .rule_index(),
            1
        );
    }

    #[test]
    fn compiled_text_program_merges_text_and_target_rules_by_original_origin() {
        let early_fill = CanvasPaint::solid("#0ea5e9").expect("valid early fill");
        let late_fill = CanvasPaint::solid("#a855f7").expect("valid late fill");
        let mut target_patch = ThemeStylePatch::default().with_fill(early_fill);
        target_patch.typography.font_size_px = Specified::Value(18.0);
        let mut text_ordinal_patch = ThemeStylePatch::default();
        text_ordinal_patch.typography.font_size_px = Specified::Value(20.0);
        let late_text_patch = ThemeStylePatch::default().with_fill(late_fill.clone());
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::NodeLabel, target_patch))
            .with_rule(
                ThemeRule::new(ThemeTarget::Text, text_ordinal_patch)
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(ThemeRule::new(ThemeTarget::Text, late_text_patch));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile text-target theme");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART).text_style(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            Some(1),
        );

        assert_eq!(resolved.fill(), Some(&late_fill));
        let fill_origin = resolved.fill_resolution().winner().unwrap();
        assert_eq!(fill_origin.rule_index(), 2);
        assert_eq!(fill_origin.target(), ThemeTarget::Text);
        assert_eq!(fill_origin.ordinal(), None);
        assert_eq!(resolved.typography().font_size_px(), 20.0);
        let font_size_origin = resolved
            .typography_resolution()
            .winner(ThemeTypographyProperty::FontSize)
            .unwrap();
        assert_eq!(font_size_origin.rule_index(), 1);
        assert_eq!(font_size_origin.target(), ThemeTarget::Text);
        assert_eq!(font_size_origin.ordinal(), Some(OrdinalSelector::Exact(1)));
    }

    #[test]
    fn metered_program_charges_only_selected_ordinal_candidates() {
        let patch = || {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#0ea5e9").expect("valid fill"))
        };
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, patch()))
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_variant(ThemeVariant::Active)
                    .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_variant(ThemeVariant::Selected)
                    .with_ordinal(OrdinalSelector::exact(3).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Text, patch())
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::NodeLabel, patch())
                    .with_variant(ThemeVariant::Active)
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            );
        let resolved = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile metered theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        resolved
            .style_with_work_meter(ThemeTarget::Node, ThemeVariant::Active, None, &work_meter)
            .expect("static-only query");
        assert_eq!(work_meter.used(), 0);

        resolved
            .style_with_work_meter(
                ThemeTarget::Node,
                ThemeVariant::Active,
                Some(99),
                &work_meter,
            )
            .expect("bounded ordinal query");
        assert_eq!(work_meter.used(), 2);

        resolved
            .text_style_with_work_meter(
                ThemeTarget::NodeLabel,
                ThemeVariant::Active,
                Some(99),
                &work_meter,
            )
            .expect("bounded text ordinal query");
        assert_eq!(work_meter.used(), 4);
    }

    #[test]
    fn rejected_ordinal_charge_does_not_advance_the_operation_meter() {
        let patch = || {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#0ea5e9").expect("valid fill"))
        };
        let rules = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            )
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, patch())
                    .with_variant(ThemeVariant::Active)
                    .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            );
        let resolved = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .expect("compile bounded theme")
            .resolve(DiagramFamilyId::FLOWCHART);
        let mut policy = RenderResourcePolicy::unbounded_for_trusted_input();
        policy
            .apply_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("set work limit");
        let work_meter = OperationWorkMeter::new(policy);

        assert!(
            resolved
                .style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Active,
                    Some(1),
                    &work_meter,
                )
                .is_err()
        );
        assert_eq!(work_meter.used(), 0);
    }

    #[test]
    fn resolved_style_equality_ignores_rule_origin_but_preserves_specified_state() {
        let fill = CanvasPaint::solid("#ef4444").expect("valid fill");
        let one_rule = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(fill.clone()),
                    ),
                )),
            )
            .expect("compile one-rule theme");
        let repeated_rule = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_fill(fill.clone()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_fill(fill),
                        )),
                ),
            )
            .expect("compile repeated-rule theme");

        let one_rule_style = one_rule.resolve(DiagramFamilyId::FLOWCHART).style(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
        );
        let repeated_rule_style = repeated_rule.resolve(DiagramFamilyId::FLOWCHART).style(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
        );

        assert_ne!(
            one_rule_style.fill_resolution().winner(),
            repeated_rule_style.fill_resolution().winner()
        );
        assert_eq!(one_rule_style, repeated_rule_style);

        let mut clear_fill = ThemeStylePatch::default();
        clear_fill.paint.fill = Specified::Clear;
        let cleared = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, clear_fill)),
            ))
            .expect("compile clear theme")
            .resolve(DiagramFamilyId::FLOWCHART)
            .style(ThemeTarget::Node, ThemeVariant::Default, None);
        let unspecified = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile empty theme")
            .resolve(DiagramFamilyId::FLOWCHART)
            .style(ThemeTarget::Node, ThemeVariant::Default, None);

        assert_eq!(cleared.fill(), unspecified.fill());
        assert_ne!(cleared, unspecified);
    }
}
