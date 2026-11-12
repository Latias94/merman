use crate::DiagramFamilyId;
use std::sync::Arc;

use super::canvas::{CanvasPaint, InsetsPx, ThemeColorValue};
use super::family_program::FamilyThemeProgram;
use super::semantic::{
    OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeRule, ThemeStylePatch, ThemeTarget,
    ThemeVariant,
};
#[cfg(test)]
use super::typography::TextStylePatch;
use super::typography::{SelectedTextStylePatch, Specified, TextStyle};

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

    pub(crate) const fn target(self) -> ThemeTarget {
        self.target
    }

    pub(crate) const fn variant(self) -> Option<ThemeVariant> {
        self.variant
    }

    pub(crate) const fn ordinal(self) -> Option<OrdinalSelector> {
        self.ordinal
    }

    #[cfg(test)]
    fn supersedes(self, current: Option<Self>) -> bool {
        current.is_none_or(|current| self.rule_index >= current.rule_index)
    }
}

/// Borrowed selected payload and its exact source, valid while the terminal style is retained.
#[derive(Debug)]
pub(crate) struct ResolvedProperty<'a, T> {
    specified: &'a Specified<T>,
    winner: Option<ThemeRuleOrigin>,
}

impl<T> Copy for ResolvedProperty<'_, T> {}

impl<T> Clone for ResolvedProperty<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: PartialEq> PartialEq for ResolvedProperty<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.specified == other.specified
    }
}

impl<T> Default for ResolvedProperty<'_, T> {
    fn default() -> Self {
        Self {
            specified: &Specified::Unspecified,
            winner: None,
        }
    }
}

impl<'a, T> ResolvedProperty<'a, T> {
    pub const fn specified(self) -> &'a Specified<T> {
        self.specified
    }

    pub(crate) const fn winner(self) -> Option<ThemeRuleOrigin> {
        self.winner
    }

    pub const fn value(self) -> Option<&'a T> {
        match self.specified {
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

impl ThemeTypographyProperty {
    pub(crate) const ALL: &'static [Self] = &[
        Self::FontStack,
        Self::FontSize,
        Self::FontWeight,
        Self::FontStyle,
        Self::LineHeight,
        Self::LetterSpacing,
        Self::WordSpacing,
        Self::Transform,
        Self::Decoration,
        Self::TextAlign,
        Self::WhiteSpace,
        Self::Wrap,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::FontStack => "font-stack",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::WordSpacing => "word-spacing",
            Self::Transform => "transform",
            Self::Decoration => "decoration",
            Self::TextAlign => "text-align",
            Self::WhiteSpace => "white-space",
            Self::Wrap => "wrap",
        }
    }
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

// One local field table for fixed selections and borrowed views. The test oracle is independent.
macro_rules! define_theme_style_selections {
    (
        style { $( $field:ident: $ty:ty => $resolution:ident, $group:ident.$patch_field:ident, $facet:ident ),+ $(,)? }
        typography { $( $text_field:ident => $property:ident ),+ $(,)? }
    ) => {
        /// Exact property winners into one immutable spec; Clear also selects a rule.
        #[derive(Debug, Clone, Copy, Default)]
        pub(super) struct ThemeStyleRuleWinners {
            $( $field: Option<usize>, )+
            $( $text_field: Option<usize>, )+
        }

        impl ThemeStyleRuleWinners {
            pub(super) fn select_rule(&mut self, patch: &ThemeStylePatch, rule_index: usize) {
                $( select_property_rule(&mut self.$field, &patch.$group.$patch_field, rule_index); )+
                $( select_property_rule(&mut self.$text_field, &patch.typography.$text_field, rule_index); )+
            }

            pub(super) fn merge_from(&mut self, other: &Self) -> bool {
                $( self.$field = self.$field.max(other.$field); )+
                let mut typography_changed = false;
                $(
                    let next = self.$text_field.max(other.$text_field);
                    typography_changed |= next != self.$text_field;
                    self.$text_field = next;
                )+
                typography_changed
            }

            fn overlay_role(&mut self, other: &Self) -> bool {
                $( self.$field = other.$field.or(self.$field); )+
                let mut typography_changed = false;
                $(
                    let next = other.$text_field.or(self.$text_field);
                    typography_changed |= next != self.$text_field;
                    self.$text_field = next;
                )+
                typography_changed
            }

            fn typography_patch<'a>(&self, spec: &'a super::DiagramThemeSpec) -> SelectedTextStylePatch<'a> {
                SelectedTextStylePatch {
                    $( $text_field: rule_property(spec, self.$text_field, |patch| &patch.typography.$text_field).specified(), )+
                }
            }

            fn typography_winner(&self, property: ThemeTypographyProperty) -> Option<usize> {
                match property {
                    $( ThemeTypographyProperty::$property => self.$text_field, )+
                }
            }
        }

        impl ResolvedThemeStyle {
            $(
                pub fn $resolution(&self) -> ResolvedProperty<'_, $ty> {
                    rule_property(&self.spec, self.winners.$field, |patch| &patch.$group.$patch_field)
                }
            )+

            pub(crate) fn winner_rule_properties(
                &self,
            ) -> impl Iterator<Item = (ResolvedStyleProperty, ThemeRuleOrigin)> + '_ {
                [
                    $( (ResolvedStyleProperty::$facet, self.winners.$field), )+
                    $( (ResolvedStyleProperty::Typography(ThemeTypographyProperty::$property), self.winners.$text_field), )+
                ]
                .into_iter()
                .filter_map(|(property, index)| index.map(|index| (property, rule_origin(&self.spec, index))))
            }
        }

        impl PartialEq for ResolvedThemeStyle {
            fn eq(&self, other: &Self) -> bool {
                // Equality compares selected content, including presence, independently of origins.
                $( self.$resolution() == other.$resolution() && )+
                    self.typography == other.typography
                    && self.spec.typography().family_style(self.family)
                        == other.spec.typography().family_style(other.family)
                    && self.typography_resolution().patch() == other.typography_resolution().patch()
            }
        }
    };
}

define_theme_style_selections! {
    style {
        fill: CanvasPaint => fill_resolution, paint.fill, Fill,
        stroke: CanvasPaint => stroke_resolution, stroke.paint, Stroke,
        stroke_width: f32 => stroke_width_resolution, stroke.width, StrokeWidth,
        stroke_dasharray: Vec<f32> => stroke_dasharray_resolution, stroke.dasharray, StrokeDasharray,
        stroke_linecap: StrokeLineCap => stroke_linecap_resolution, stroke.linecap, StrokeLinecap,
        stroke_linejoin: StrokeLineJoin => stroke_linejoin_resolution, stroke.linejoin, StrokeLinejoin,
        opacity: f32 => opacity_resolution, paint.opacity, Opacity,
        fill_opacity: f32 => fill_opacity_resolution, paint.fill_opacity, FillOpacity,
        stroke_opacity: f32 => stroke_opacity_resolution, stroke.stroke_opacity, StrokeOpacity,
        radius: f32 => radius_resolution, geometry.radius, Radius,
        padding: InsetsPx => padding_resolution, spacing.padding, Padding,
        effect: String => effect_resolution, effects.effect, Effect,
    }
    typography {
        font_stack => FontStack,
        font_size_px => FontSize,
        font_weight => FontWeight,
        font_style => FontStyle,
        line_height => LineHeight,
        letter_spacing_px => LetterSpacing,
        word_spacing_px => WordSpacing,
        transform => Transform,
        decoration => Decoration,
        text_align => TextAlign,
        white_space => WhiteSpace,
        wrap => Wrap,
    }
}

fn select_property_rule<T>(
    winner: &mut Option<usize>,
    specified: &Specified<T>,
    rule_index: usize,
) {
    if !specified.is_unspecified() && winner.is_none_or(|current| rule_index >= current) {
        *winner = Some(rule_index);
    }
}

fn rule_origin(spec: &super::DiagramThemeSpec, rule_index: usize) -> ThemeRuleOrigin {
    ThemeRuleOrigin::new(rule_index, &spec.styles().rules()[rule_index])
}

fn rule_property<'a, T>(
    spec: &'a super::DiagramThemeSpec,
    winner: Option<usize>,
    property: impl FnOnce(&ThemeStylePatch) -> &Specified<T>,
) -> ResolvedProperty<'a, T> {
    match winner {
        Some(rule_index) => ResolvedProperty {
            specified: property(spec.styles().rules()[rule_index].style()),
            winner: Some(rule_origin(spec, rule_index)),
        },
        None => ResolvedProperty::default(),
    }
}

/// A transient view; only the effective typography is owned by the terminal style.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedTypography<'a> {
    style: &'a ResolvedThemeStyle,
}

impl<'a> ResolvedTypography<'a> {
    pub const fn computed(self) -> &'a TextStyle {
        &self.style.typography
    }

    pub fn patch(self) -> SelectedTextStylePatch<'a> {
        self.style.winners.typography_patch(&self.style.spec)
    }

    pub(crate) fn winner(self, property: ThemeTypographyProperty) -> Option<ThemeRuleOrigin> {
        self.style
            .winners
            .typography_winner(property)
            .map(|index| rule_origin(&self.style.spec, index))
    }
}

/// Retains the admitted spec once, exact selections, and the effective native text style.
/// The owner is the spec, never the compiled wrapper or family cache, so no cycle is formed.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedThemeStyle {
    spec: Arc<super::DiagramThemeSpec>,
    family: DiagramFamilyId,
    winners: ThemeStyleRuleWinners,
    typography: TextStyle,
}

impl ResolvedThemeStyle {
    pub(super) fn from_rule_winners(
        spec: Arc<super::DiagramThemeSpec>,
        family: DiagramFamilyId,
        base_typography: &TextStyle,
        winners: ThemeStyleRuleWinners,
    ) -> Self {
        let mut typography = base_typography.clone();
        winners
            .typography_patch(&spec)
            .apply_to(&mut typography, base_typography);
        Self {
            spec,
            family,
            winners,
            typography,
        }
    }

    // All production merges select roles/variants from the same ResolvedDiagramTheme. Enforce
    // that boundary rather than interpreting another recipe's local indices in this spec.
    fn assert_same_spec(&self, other: &Self) {
        assert!(
            Arc::ptr_eq(&self.spec, &other.spec),
            "resolved style merge requires the same theme spec"
        );
    }

    pub(crate) fn merge_from(&mut self, other: &Self) {
        self.assert_same_spec(other);
        if self.winners.merge_from(&other.winners) {
            self.recompute_typography();
        }
    }

    /// A specific role overrides only its specified properties, independently of source order.
    pub(crate) fn overlay_role(&mut self, other: Self) {
        self.assert_same_spec(&other);
        if self.winners.overlay_role(&other.winners) {
            self.recompute_typography();
        }
    }

    fn recompute_typography(&mut self) {
        // Clear always restores the receiver's family base, even when the incoming family differs.
        let base = self.spec.typography().family_style(self.family);
        self.typography = base.clone();
        self.winners
            .typography_patch(&self.spec)
            .apply_to(&mut self.typography, &base);
    }

    pub fn fill(&self) -> Option<&CanvasPaint> {
        self.fill_resolution().value()
    }

    pub fn stroke(&self) -> Option<&CanvasPaint> {
        self.stroke_resolution().value()
    }

    pub fn stroke_width(&self) -> Option<f32> {
        self.stroke_width_resolution().value().copied()
    }

    pub fn stroke_dasharray(&self) -> Option<&[f32]> {
        self.stroke_dasharray_resolution()
            .value()
            .map(Vec::as_slice)
    }

    pub fn stroke_linecap(&self) -> Option<StrokeLineCap> {
        self.stroke_linecap_resolution().value().copied()
    }

    pub fn stroke_linejoin(&self) -> Option<StrokeLineJoin> {
        self.stroke_linejoin_resolution().value().copied()
    }

    pub fn opacity(&self) -> Option<f32> {
        self.opacity_resolution().value().copied()
    }

    pub fn fill_opacity(&self) -> Option<f32> {
        self.fill_opacity_resolution().value().copied()
    }

    pub fn stroke_opacity(&self) -> Option<f32> {
        self.stroke_opacity_resolution().value().copied()
    }

    pub fn radius(&self) -> Option<f32> {
        self.radius_resolution().value().copied()
    }

    pub fn padding(&self) -> Option<InsetsPx> {
        self.padding_resolution().value().copied()
    }

    pub const fn typography(&self) -> &TextStyle {
        self.typography_resolution().computed()
    }

    pub const fn typography_resolution(&self) -> ResolvedTypography<'_> {
        ResolvedTypography { style: self }
    }
}

#[cfg(test)]
mod oracle {
    use super::*;
    use std::collections::BTreeMap;

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

        fn winner(&self) -> Option<ThemeRuleOrigin> {
            self.winner
        }
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

        #[cfg(test)]
        fn apply(&mut self, patch: &TextStylePatch, origin: ThemeRuleOrigin) {
            let mut changed = false;
            changed |= apply_typography_property(
                &patch.font_stack,
                &mut self.patch.font_stack,
                ThemeTypographyProperty::FontStack,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.font_size_px,
                &mut self.patch.font_size_px,
                ThemeTypographyProperty::FontSize,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.font_weight,
                &mut self.patch.font_weight,
                ThemeTypographyProperty::FontWeight,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.font_style,
                &mut self.patch.font_style,
                ThemeTypographyProperty::FontStyle,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.line_height,
                &mut self.patch.line_height,
                ThemeTypographyProperty::LineHeight,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.letter_spacing_px,
                &mut self.patch.letter_spacing_px,
                ThemeTypographyProperty::LetterSpacing,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.word_spacing_px,
                &mut self.patch.word_spacing_px,
                ThemeTypographyProperty::WordSpacing,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.transform,
                &mut self.patch.transform,
                ThemeTypographyProperty::Transform,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.decoration,
                &mut self.patch.decoration,
                ThemeTypographyProperty::Decoration,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.text_align,
                &mut self.patch.text_align,
                ThemeTypographyProperty::TextAlign,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.white_space,
                &mut self.patch.white_space,
                ThemeTypographyProperty::WhiteSpace,
                origin,
                &mut self.winners,
            );
            changed |= apply_typography_property(
                &patch.wrap,
                &mut self.patch.wrap,
                ThemeTypographyProperty::Wrap,
                origin,
                &mut self.winners,
            );
            if changed {
                self.computed = self.base.clone();
                self.patch.apply_to(&mut self.computed, &self.base);
            }
        }

        fn winner(&self, property: ThemeTypographyProperty) -> Option<ThemeRuleOrigin> {
            self.winners.get(&property).copied()
        }
    }

    fn apply_typography_property<T: Clone>(
        incoming: &Specified<T>,
        resolved: &mut Specified<T>,
        property: ThemeTypographyProperty,
        origin: ThemeRuleOrigin,
        winners: &mut BTreeMap<ThemeTypographyProperty, ThemeRuleOrigin>,
    ) -> bool {
        if incoming.is_unspecified() || !origin.supersedes(winners.get(&property).copied()) {
            return false;
        }
        *resolved = incoming.clone();
        winners.insert(property, origin);
        true
    }

    #[derive(Debug, Clone, PartialEq)]
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
    }

    impl ResolvedThemeStyle {
        #[cfg(test)]
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
            }
        }

        #[cfg(test)]
        pub(super) fn apply(&mut self, patch: &ThemeStylePatch, origin: ThemeRuleOrigin) {
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

        pub(crate) fn winner_rule_properties(
            &self,
        ) -> impl Iterator<Item = (ResolvedStyleProperty, ThemeRuleOrigin)> + '_ {
            [
                (ResolvedStyleProperty::Fill, self.fill.winner()),
                (ResolvedStyleProperty::Stroke, self.stroke.winner()),
                (
                    ResolvedStyleProperty::StrokeWidth,
                    self.stroke_width.winner(),
                ),
                (
                    ResolvedStyleProperty::StrokeDasharray,
                    self.stroke_dasharray.winner(),
                ),
                (
                    ResolvedStyleProperty::StrokeLinecap,
                    self.stroke_linecap.winner(),
                ),
                (
                    ResolvedStyleProperty::StrokeLinejoin,
                    self.stroke_linejoin.winner(),
                ),
                (ResolvedStyleProperty::Opacity, self.opacity.winner()),
                (
                    ResolvedStyleProperty::FillOpacity,
                    self.fill_opacity.winner(),
                ),
                (
                    ResolvedStyleProperty::StrokeOpacity,
                    self.stroke_opacity.winner(),
                ),
                (ResolvedStyleProperty::Radius, self.radius.winner()),
                (ResolvedStyleProperty::Padding, self.padding.winner()),
                (ResolvedStyleProperty::Effect, self.effect.winner()),
            ]
            .into_iter()
            .filter_map(|(property, origin)| origin.map(|origin| (property, origin)))
            .chain(
                [
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
                ]
                .into_iter()
                .filter_map(|property| {
                    self.typography
                        .winner(property)
                        .map(|origin| (ResolvedStyleProperty::Typography(property), origin))
                }),
            )
        }
    }

    impl PartialEq<ResolvedThemeStyle> for super::ResolvedThemeStyle {
        fn eq(&self, other: &ResolvedThemeStyle) -> bool {
            self.fill_resolution().specified() == &other.fill.specified
                && self.stroke_resolution().specified() == &other.stroke.specified
                && self.stroke_width_resolution().specified() == &other.stroke_width.specified
                && self.stroke_dasharray_resolution().specified()
                    == &other.stroke_dasharray.specified
                && self.stroke_linecap_resolution().specified() == &other.stroke_linecap.specified
                && self.stroke_linejoin_resolution().specified() == &other.stroke_linejoin.specified
                && self.opacity_resolution().specified() == &other.opacity.specified
                && self.fill_opacity_resolution().specified() == &other.fill_opacity.specified
                && self.stroke_opacity_resolution().specified() == &other.stroke_opacity.specified
                && self.radius_resolution().specified() == &other.radius.specified
                && self.padding_resolution().specified() == &other.padding.specified
                && self.effect_resolution().specified() == &other.effect.specified
                && self.typography == other.typography.computed
                && self.spec.typography().family_style(self.family) == other.typography.base
                && self.typography_resolution().patch()
                    == SelectedTextStylePatch::from(&other.typography.patch)
        }
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

    pub(crate) fn family_program_owner(&self) -> Arc<FamilyThemeProgram> {
        Arc::clone(&self.program)
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
        resolution: ResolvedProperty<'_, String>,
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

    pub(crate) fn family_mechanism_key(
        &self,
        route: super::family_mechanism_matrix::FamilyThemeRoute,
    ) -> super::application::FamilyThemeMechanismKey {
        self.program.mechanism_key(route)
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

    /// Retain a family's historical variant while admitting an additional semantic role.
    /// Property winners keep author order across both variants, including explicit Clear.
    pub(crate) fn text_style_with_variant_fallback(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        fallback: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, crate::resources::OperationWorkError> {
        let mut style = self.text_style_with_work_meter(target, fallback, ordinal, work_meter)?;
        if variant != fallback {
            style.merge_from(
                &self.text_style_with_work_meter(target, variant, ordinal, work_meter)?,
            );
        }
        Ok(style)
    }

    /// Preserve the existing Text/fallback author order, then override specified role facets.
    pub(crate) fn text_style_with_role_override(
        &self,
        fallback_target: ThemeTarget,
        override_target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<ResolvedThemeStyle, crate::resources::OperationWorkError> {
        self.program.resolve_text_style_with_role_override(
            fallback_target,
            override_target,
            variant,
            ordinal,
            work_meter,
        )
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
) -> oracle::ResolvedThemeStyle {
    let base_typography = spec.typography().family_style(family);
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
) -> oracle::ResolvedThemeStyle {
    let mut style = oracle::ResolvedThemeStyle::new(base_typography.clone());
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
    fn typography_recomputation_preserves_every_property_and_winner() {
        use crate::diagram_theme::{
            FontStack, FontStyle, LineHeight, TextAlign, TextDecoration, TextTransform,
            ThemeWrapMode as WrapMode, WhiteSpace,
        };
        let values = TextStylePatch {
            font_stack: Specified::Value(FontStack::new(["Example", "serif"]).unwrap()),
            font_size_px: Specified::Value(23.0),
            font_weight: Specified::Value(700),
            font_style: Specified::Value(FontStyle::Italic),
            line_height: Specified::Value(LineHeight::Multiplier(1.5)),
            letter_spacing_px: Specified::Value(2.0),
            word_spacing_px: Specified::Value(3.0),
            transform: Specified::Value(TextTransform::Uppercase),
            decoration: Specified::Value(TextDecoration::Underline),
            text_align: Specified::Value(TextAlign::Center),
            white_space: Specified::Value(WhiteSpace::PreWrap),
            wrap: Specified::Value(WrapMode::Anywhere),
        };
        let clears = TextStylePatch {
            font_stack: Specified::Clear,
            font_size_px: Specified::Clear,
            font_weight: Specified::Clear,
            font_style: Specified::Clear,
            line_height: Specified::Clear,
            letter_spacing_px: Specified::Clear,
            word_spacing_px: Specified::Clear,
            transform: Specified::Clear,
            decoration: Specified::Clear,
            text_align: Specified::Clear,
            white_space: Specified::Clear,
            wrap: Specified::Clear,
        };
        let base = TextStyle::default()
            .with_font_stack(FontStack::new(["Base Font", "sans-serif"]).unwrap())
            .with_font_size_px(16.0)
            .unwrap();
        // Exercise selection, source-order merge, and role overlay for every text field.
        macro_rules! check_property {
            ($field:ident, $property:ident) => {{
                let value = TextStylePatch {
                    $field: values.$field.clone(),
                    ..Default::default()
                };
                let clear = TextStylePatch {
                    $field: Specified::Clear,
                    ..Default::default()
                };
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new()
                            .with_typography(
                                super::super::TypographySpec::default().with_default(base.clone()),
                            )
                            .with_styles(
                                ThemeRuleSet::default()
                                    .with_rule(
                                        ThemeRule::new(
                                            ThemeTarget::Text,
                                            ThemeStylePatch {
                                                typography: clear.clone(),
                                                ..Default::default()
                                            },
                                        )
                                        .with_variant(ThemeVariant::Warning),
                                    )
                                    .with_rule(
                                        ThemeRule::new(
                                            ThemeTarget::Text,
                                            ThemeStylePatch {
                                                typography: value.clone(),
                                                ..Default::default()
                                            },
                                        )
                                        .with_variant(ThemeVariant::Active),
                                    )
                                    .with_rule(
                                        ThemeRule::new(
                                            ThemeTarget::Text,
                                            ThemeStylePatch {
                                                typography: clear,
                                                ..Default::default()
                                            },
                                        )
                                        .with_variant(ThemeVariant::Selected),
                                    ),
                            ),
                    )
                    .unwrap()
                    .resolve(DiagramFamilyId::FLOWCHART);
                let mut expected = base.clone();
                value.apply_to(&mut expected, &base);
                let direct = theme.style(ThemeTarget::Text, ThemeVariant::Active, None);
                let clear = theme.style(ThemeTarget::Text, ThemeVariant::Selected, None);
                let mut merged = theme.style(ThemeTarget::Text, ThemeVariant::Default, None);
                merged.merge_from(&direct);
                assert_eq!(direct.typography(), &expected, stringify!($field));
                assert_eq!(merged, direct);
                assert_eq!(
                    merged.typography_resolution().patch().$field,
                    &values.$field
                );
                assert_eq!(
                    merged
                        .typography_resolution()
                        .winner(ThemeTypographyProperty::$property)
                        .unwrap()
                        .rule_index(),
                    1
                );
                merged.merge_from(&theme.style(ThemeTarget::Text, ThemeVariant::Warning, None));
                assert_eq!(merged, direct, "losing Clear: {}", stringify!($field));
                merged.merge_from(&direct);
                assert_eq!(merged, direct, "equal-index merge: {}", stringify!($field));
                merged.merge_from(&clear);
                assert_eq!(merged.typography(), &base, stringify!($field));
                assert_eq!(
                    merged.typography_resolution().patch().$field,
                    &Specified::Clear
                );
                // A losing value and an unspecified input retain Clear and its origin.
                merged.merge_from(&direct);
                merged.merge_from(&theme.style(ThemeTarget::Text, ThemeVariant::Default, None));
                assert_eq!(merged.typography(), &base, stringify!($field));
                assert_eq!(
                    merged
                        .typography_resolution()
                        .winner(ThemeTypographyProperty::$property)
                        .unwrap()
                        .rule_index(),
                    2
                );
                // Role priority overrides source order, including equal-valued origins.
                merged.overlay_role(direct.clone());
                assert_eq!(merged, direct);
                merged.overlay_role(clear.clone());
                assert_eq!(merged, clear);
            }};
        }
        check_property!(font_stack, FontStack);
        check_property!(font_size_px, FontSize);
        check_property!(font_weight, FontWeight);
        check_property!(font_style, FontStyle);
        check_property!(line_height, LineHeight);
        check_property!(letter_spacing_px, LetterSpacing);
        check_property!(word_spacing_px, WordSpacing);
        check_property!(transform, Transform);
        check_property!(decoration, Decoration);
        check_property!(text_align, TextAlign);
        check_property!(white_space, WhiteSpace);
        check_property!(wrap, Wrap);

        // The independent oracle must also cover every property through compiled selection.
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch {
                    typography: values.clone(),
                    ..Default::default()
                },
            ))
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch {
                        typography: clears,
                        ..Default::default()
                    },
                )
                .with_variant(ThemeVariant::Active),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch {
                        typography: values,
                        ..Default::default()
                    },
                )
                .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(super::super::TypographySpec::default().with_default(base))
                    .with_styles(rules),
            )
            .unwrap();
        let family = theme.resolve(DiagramFamilyId::FLOWCHART);
        for variant in [ThemeVariant::Default, ThemeVariant::Active] {
            for ordinal in [None, Some(1), Some(2)] {
                let indexed = family.style(ThemeTarget::Node, variant, ordinal);
                let expected = resolve_style(
                    theme.spec(),
                    DiagramFamilyId::FLOWCHART,
                    ThemeTarget::Node,
                    variant,
                    ordinal,
                );
                assert_eq!(indexed, expected);
                assert_eq!(
                    indexed.winner_rule_properties().collect::<Vec<_>>(),
                    expected.winner_rule_properties().collect::<Vec<_>>(),
                );
            }
        }
    }

    #[test]
    fn typography_merge_recomputes_only_with_the_receivers_base() {
        let base = TextStyle::default().with_font_size_px(19.0).unwrap();
        let foreign_base = TextStyle::default().with_font_size_px(31.0).unwrap();
        // Different family bases still select from one admitted spec; local indices stay valid.
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(
                        super::super::TypographySpec::default()
                            .with_default(base.clone())
                            .with_family_style(DiagramFamilyId::STATE, foreign_base.clone()),
                    )
                    .with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch {
                                    typography: TextStylePatch {
                                        font_size_px: Specified::Clear,
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                },
                            )
                            .with_variant(ThemeVariant::Active),
                        ),
                    ),
            )
            .unwrap();
        let receiver = theme.resolve(DiagramFamilyId::FLOWCHART);
        let incoming = theme.resolve(DiagramFamilyId::STATE);
        let mut resolved = receiver.style(ThemeTarget::Text, ThemeVariant::Default, None);
        let unspecified = incoming.style(ThemeTarget::Text, ThemeVariant::Default, None);
        assert_eq!(unspecified.typography(), &foreign_base);
        resolved.merge_from(&unspecified);
        assert_eq!(resolved.typography(), &base);
        let clear = incoming.style(ThemeTarget::Text, ThemeVariant::Active, None);
        resolved.merge_from(&clear);
        assert_eq!(resolved.typography(), &base);
        assert_eq!(
            resolved.typography_resolution().patch().font_size_px,
            &Specified::Clear
        );
        resolved.overlay_role(clear);
        assert_eq!(resolved.typography(), &base);
    }

    #[test]
    fn merges_reject_foreign_recipe_indices_before_changing_selections() {
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )));
        let style = |spec| {
            DiagramThemeCompiler::new()
                .compile(spec)
                .unwrap()
                .resolve(DiagramFamilyId::FLOWCHART)
                .style(ThemeTarget::Text, ThemeVariant::Default, None)
        };
        let mut receiver = style(spec.clone());
        let foreign = style(spec);
        assert_eq!(receiver, foreign);
        let before = receiver.clone();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                receiver.merge_from(&foreign);
            }))
            .is_err()
        );
        assert_eq!(receiver, before);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                receiver.overlay_role(foreign);
            }))
            .is_err()
        );
        assert_eq!(receiver, before);
    }

    #[test]
    fn terminal_clones_borrow_exact_payloads_and_retain_only_the_spec_owner() {
        use crate::diagram_theme::{FontStack, GradientStop, LinearGradient};

        let fill = CanvasPaint::LinearGradient(
            LinearGradient::new(
                90.0,
                [
                    GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                    GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
                ],
            )
            .unwrap(),
        );
        let mut patch = ThemeStylePatch::default()
            .with_fill(fill.clone())
            .with_stroke(CanvasPaint::solid("#456789").unwrap())
            .with_stroke_dasharray([3.0, 5.0])
            .unwrap()
            .with_effect("shadow")
            .unwrap();
        patch.stroke.width = Specified::Value(2.0);
        patch.stroke.linecap = Specified::Value(StrokeLineCap::Round);
        patch.stroke.linejoin = Specified::Value(StrokeLineJoin::Bevel);
        patch.paint.opacity = Specified::Value(0.8);
        patch.paint.fill_opacity = Specified::Value(0.6);
        patch.stroke.stroke_opacity = Specified::Value(0.4);
        patch.geometry.radius = Specified::Value(4.0);
        patch.spacing.padding = Specified::Value(InsetsPx::all(6.0));
        patch.typography.font_stack =
            Specified::Value(FontStack::new(["Example", "serif"]).unwrap());
        patch.typography.font_size_px = Specified::Value(24.0);
        let graph = EffectGraph::new(
            "shadow",
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#00000080").unwrap(),
            }],
        )
        .unwrap();
        // The later equal fill must retain its own origin, without replaying its losing siblings.
        let mut equal_fill = ThemeStylePatch::default().with_fill(fill);
        equal_fill.typography.font_size_px = Specified::Value(24.0);
        let spec = DiagramThemeSpec::new()
            .with_effects(DiagramEffectSet::default().with_graph(graph).unwrap())
            .with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(ThemeTarget::Node, patch))
                    .with_rule(ThemeRule::new(ThemeTarget::Node, equal_fill)),
            );
        let equivalent = DiagramThemeCompiler::new()
            .compile(spec.clone())
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART)
            .style(ThemeTarget::Node, ThemeVariant::Default, None);
        let (style, owner) = {
            let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
            let family = theme.resolve(DiagramFamilyId::FLOWCHART);
            let style = family.style(ThemeTarget::Node, ThemeVariant::Default, None);
            assert!(std::ptr::eq(style.spec.as_ref(), theme.spec()));
            let expected = resolve_style(
                theme.spec(),
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                ThemeVariant::Default,
                None,
            );
            assert_eq!(style, expected);
            assert_eq!(
                style.winner_rule_properties().collect::<Vec<_>>(),
                expected.winner_rule_properties().collect::<Vec<_>>()
            );
            let owner = Arc::downgrade(&style.spec);
            (style, owner)
        };
        assert_eq!(style, equivalent);
        let retained = style.clone();
        assert!(Arc::ptr_eq(&style.spec, &retained.spec));
        assert!(std::ptr::eq(
            style.fill().unwrap(),
            retained.fill().unwrap()
        ));
        drop(style);
        let rules = retained.spec.styles().rules();
        assert!(std::ptr::eq(
            retained.fill_resolution().specified(),
            &rules[1].style().paint.fill
        ));
        assert!(std::ptr::eq(
            retained.stroke_resolution().specified(),
            &rules[0].style().stroke.paint
        ));
        assert!(std::ptr::eq(
            retained.stroke_dasharray_resolution().specified(),
            &rules[0].style().stroke.dasharray
        ));
        assert!(std::ptr::eq(
            retained.effect_resolution().specified(),
            &rules[0].style().effects.effect
        ));
        assert!(std::ptr::eq(
            retained.typography_resolution().patch().font_stack,
            &rules[0].style().typography.font_stack
        ));
        assert!(std::ptr::eq(
            retained.typography_resolution().patch().font_size_px,
            &rules[1].style().typography.font_size_px
        ));
        assert_eq!(retained.stroke_dasharray(), Some([3.0, 5.0].as_slice()));
        assert_eq!(retained.typography().font_size_px(), 24.0);
        assert_eq!(retained.fill_resolution().winner().unwrap().rule_index(), 1);
        assert_eq!(
            retained.effect_resolution().winner().unwrap().rule_index(),
            0
        );
        assert!(owner.upgrade().is_some());
        drop(retained);
        assert!(
            owner.upgrade().is_none(),
            "terminal retention must not create an owner cycle"
        );
    }

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
    fn compiled_program_preserves_split_winners_when_later_rules_clear_other_fields() {
        let stroke = CanvasPaint::solid("#2563eb").expect("valid stroke");
        let mut early = ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ef4444").unwrap())
            .with_stroke(stroke.clone())
            .with_stroke_dasharray([3.0, 5.0])
            .unwrap();
        early.typography.font_size_px = Specified::Value(24.0);
        early.typography.font_weight = Specified::Value(700);
        let mut late = ThemeStylePatch::default();
        late.paint.fill = Specified::Clear;
        late.stroke.dasharray = Specified::Clear;
        late.typography.font_size_px = Specified::Clear;
        let mut ordinal = ThemeStylePatch::default();
        ordinal.typography.font_weight = Specified::Clear;
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, early).with_variant(ThemeVariant::Active))
            .with_rule(ThemeRule::new(ThemeTarget::Node, late))
            .with_rule(
                ThemeRule::new(ThemeTarget::Node, ordinal)
                    .with_variant(ThemeVariant::Active)
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            );
        // The owning result must also survive the recipe and program being dropped.
        let style = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART)
            .style(ThemeTarget::Node, ThemeVariant::Active, Some(1));

        assert_eq!(style.stroke(), Some(&stroke));
        assert_eq!(style.stroke_resolution().winner().unwrap().rule_index(), 0);
        assert_eq!(style.fill_resolution().specified(), &Specified::Clear);
        assert_eq!(style.fill_resolution().winner().unwrap().rule_index(), 1);
        assert_eq!(
            style.stroke_dasharray_resolution().specified(),
            &Specified::Clear
        );
        assert_eq!(
            style
                .stroke_dasharray_resolution()
                .winner()
                .unwrap()
                .rule_index(),
            1
        );
        assert_eq!(style.typography(), &TextStyle::default());
        let typography = style.typography_resolution();
        assert_eq!(typography.patch().font_size_px, &Specified::Clear);
        assert_eq!(typography.patch().font_weight, &Specified::Clear);
        assert_eq!(
            typography
                .winner(ThemeTypographyProperty::FontSize)
                .unwrap()
                .rule_index(),
            1
        );
        assert_eq!(
            typography
                .winner(ThemeTypographyProperty::FontWeight)
                .unwrap()
                .rule_index(),
            2
        );
        assert_eq!(
            style.stroke_width_resolution().specified(),
            &Specified::Unspecified
        );
        assert_eq!(style.stroke_width_resolution().winner(), None);
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
    fn role_override_keeps_clear_equal_value_origins_and_unspecified_siblings() {
        let fill = CanvasPaint::solid("#123456").unwrap();
        let stroke = CanvasPaint::solid("#abcdef").unwrap();
        let mut role = ThemeStylePatch::default().with_stroke(stroke.clone());
        role.paint.fill = Specified::Clear;
        role.stroke.dasharray = Specified::Clear;
        role.effects.effect = Specified::Clear;
        role.typography.font_weight = Specified::Clear;
        let mut text = ThemeStylePatch::default().with_fill(fill.clone());
        text.paint.opacity = Specified::Value(0.6);
        text.typography.font_size_px = Specified::Value(22.0);
        text.typography.font_weight = Specified::Value(700);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(ThemeTarget::ClusterLabel, role.clone())
                                .with_variant(ThemeVariant::Default),
                        )
                        .with_rule(
                            ThemeRule::new(ThemeTarget::ClusterLabel, {
                                role.paint.fill = Specified::Value(fill.clone());
                                role
                            })
                            .with_variant(ThemeVariant::Active),
                        )
                        .with_rule(ThemeRule::new(ThemeTarget::Text, text)),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for (variant, rule_index) in [(ThemeVariant::Default, 0), (ThemeVariant::Active, 1)] {
            let fallback = theme.text_style(ThemeTarget::Title, variant, None);
            assert_eq!(fallback.fill(), Some(&fill));
            assert_eq!(fallback.fill_resolution().winner().unwrap().rule_index(), 2);
            let overlaid = theme
                .text_style_with_role_override(
                    ThemeTarget::Title,
                    ThemeTarget::ClusterLabel,
                    variant,
                    None,
                    &meter,
                )
                .unwrap();
            assert_eq!(
                overlaid.fill(),
                (variant == ThemeVariant::Active).then_some(&fill)
            );
            assert_eq!(
                overlaid.fill_resolution().winner().unwrap().rule_index(),
                rule_index
            );
            assert_eq!(
                overlaid.fill_resolution().winner().unwrap().target(),
                ThemeTarget::ClusterLabel
            );
            assert_eq!(overlaid.stroke(), Some(&stroke));
            assert_eq!(
                overlaid.stroke_dasharray_resolution().specified(),
                &Specified::Clear
            );
            assert_eq!(overlaid.effect_resolution().specified(), &Specified::Clear);
            assert_eq!(overlaid.opacity(), Some(0.6));
            assert_eq!(
                overlaid.opacity_resolution().winner().unwrap().rule_index(),
                2
            );
            assert_eq!(overlaid.typography().font_size_px(), 22.0);
            assert_eq!(
                overlaid.typography().font_weight(),
                TextStyle::default().font_weight()
            );
            assert_eq!(
                overlaid
                    .typography_resolution()
                    .winner(ThemeTypographyProperty::FontWeight)
                    .unwrap()
                    .rule_index(),
                rule_index
            );
        }
        assert_eq!(meter.used(), 0);
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
