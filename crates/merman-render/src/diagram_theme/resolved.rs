use crate::render_family::RenderFamilyKind;
use std::collections::BTreeMap;

use super::canvas::{CanvasPaint, InsetsPx, ThemeColorValue};
use super::compiler::ThemeCapabilityReport;
use super::semantic::{
    StrokeLineCap, StrokeLineJoin, ThemeRule, ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use super::typography::{Specified, TextStyle};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedThemeStyle {
    fill: Option<CanvasPaint>,
    stroke: Option<CanvasPaint>,
    stroke_width: Option<f32>,
    stroke_dasharray: Option<Vec<f32>>,
    stroke_linecap: Option<StrokeLineCap>,
    stroke_linejoin: Option<StrokeLineJoin>,
    opacity: Option<f32>,
    fill_opacity: Option<f32>,
    stroke_opacity: Option<f32>,
    radius: Option<f32>,
    padding: Option<InsetsPx>,
    typography: TextStyle,
    effect: Option<String>,
}

impl ResolvedThemeStyle {
    fn new(typography: TextStyle) -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: None,
            stroke_dasharray: None,
            stroke_linecap: None,
            stroke_linejoin: None,
            opacity: None,
            fill_opacity: None,
            stroke_opacity: None,
            radius: None,
            padding: None,
            typography,
            effect: None,
        }
    }

    fn apply(&mut self, patch: &ThemeStylePatch, base_typography: &TextStyle) {
        apply_optional(&patch.paint.fill, &mut self.fill);
        apply_optional(&patch.stroke.paint, &mut self.stroke);
        apply_optional(&patch.stroke.width, &mut self.stroke_width);
        apply_optional(&patch.stroke.dasharray, &mut self.stroke_dasharray);
        apply_optional(&patch.stroke.linecap, &mut self.stroke_linecap);
        apply_optional(&patch.stroke.linejoin, &mut self.stroke_linejoin);
        apply_optional(&patch.paint.opacity, &mut self.opacity);
        apply_optional(&patch.paint.fill_opacity, &mut self.fill_opacity);
        apply_optional(&patch.stroke.stroke_opacity, &mut self.stroke_opacity);
        apply_optional(&patch.geometry.radius, &mut self.radius);
        apply_optional(&patch.spacing.padding, &mut self.padding);
        patch
            .typography
            .apply_to(&mut self.typography, base_typography);
        apply_optional(&patch.effects.effect, &mut self.effect);
    }

    pub const fn fill(&self) -> Option<&CanvasPaint> {
        self.fill.as_ref()
    }

    pub const fn stroke(&self) -> Option<&CanvasPaint> {
        self.stroke.as_ref()
    }

    pub const fn stroke_width(&self) -> Option<f32> {
        self.stroke_width
    }

    pub fn stroke_dasharray(&self) -> Option<&[f32]> {
        self.stroke_dasharray.as_deref()
    }

    pub const fn stroke_linecap(&self) -> Option<StrokeLineCap> {
        self.stroke_linecap
    }

    pub const fn stroke_linejoin(&self) -> Option<StrokeLineJoin> {
        self.stroke_linejoin
    }

    pub const fn opacity(&self) -> Option<f32> {
        self.opacity
    }

    pub const fn fill_opacity(&self) -> Option<f32> {
        self.fill_opacity
    }

    pub const fn stroke_opacity(&self) -> Option<f32> {
        self.stroke_opacity
    }

    pub const fn radius(&self) -> Option<f32> {
        self.radius
    }

    pub const fn padding(&self) -> Option<InsetsPx> {
        self.padding
    }

    pub const fn typography(&self) -> &TextStyle {
        &self.typography
    }

    pub fn effect(&self) -> Option<&str> {
        self.effect.as_deref()
    }
}

/// Immutable family projection of a compiled diagram theme. It carries no mutable renderer state.
#[derive(Debug, Clone)]
pub struct ResolvedDiagramTheme {
    theme: super::DiagramTheme,
    family: RenderFamilyKind,
    base_typography: TextStyle,
    rule_indices: BTreeMap<ThemeTarget, Vec<usize>>,
    ordinal_palette_indices: BTreeMap<ThemeTarget, usize>,
}

impl ResolvedDiagramTheme {
    pub(crate) fn new(theme: super::DiagramTheme, family: RenderFamilyKind) -> Self {
        let spec = theme.spec();
        let base_typography = spec.typography().family_style(family).clone();
        let mut rule_indices = BTreeMap::<ThemeTarget, Vec<usize>>::new();
        for (index, rule) in spec.styles().rules().iter().enumerate() {
            if rule.target().valid_for(family)
                && rule.family().is_none_or(|expected| expected == family)
            {
                rule_indices.entry(rule.target()).or_default().push(index);
            }
        }
        let ordinal_palette_indices = spec
            .styles()
            .ordinal_palettes()
            .iter()
            .enumerate()
            .filter_map(|(index, (target, _))| target.valid_for(family).then_some((*target, index)))
            .collect();

        Self {
            theme,
            family,
            base_typography,
            rule_indices,
            ordinal_palette_indices,
        }
    }

    pub const fn family(&self) -> RenderFamilyKind {
        self.family
    }

    pub const fn typography(&self) -> &TextStyle {
        &self.base_typography
    }

    pub fn style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        let Some(indices) = self.rule_indices.get(&target) else {
            return ResolvedThemeStyle::new(self.base_typography.clone());
        };
        let rules = self.theme.spec().styles().rules();
        resolve_matching_rules(
            &self.base_typography,
            indices
                .iter()
                .map(|index| &rules[*index])
                .filter(|rule| rule.applies_to(self.family, variant, ordinal)),
        )
    }

    pub fn series_color(
        &self,
        target: ThemeTarget,
        one_based_index: usize,
    ) -> Option<&ThemeColorValue> {
        let index = *self.ordinal_palette_indices.get(&target)?;
        self.theme.spec().styles().ordinal_palettes()[index]
            .1
            .color_for(one_based_index)
    }

    pub fn capabilities(&self) -> &ThemeCapabilityReport {
        self.theme.capabilities()
    }
}

/// Resolves directly from an uncompiled spec for Mermaid compatibility projection.
pub(crate) fn resolve_style(
    spec: &super::DiagramThemeSpec,
    family: RenderFamilyKind,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
) -> ResolvedThemeStyle {
    let base_typography = spec.typography().family_style(family).clone();
    resolve_matching_rules(
        &base_typography,
        spec.styles()
            .matching_rules(family, target, variant, ordinal),
    )
}

fn resolve_matching_rules<'a>(
    base_typography: &TextStyle,
    rules: impl Iterator<Item = &'a ThemeRule>,
) -> ResolvedThemeStyle {
    let mut style = ResolvedThemeStyle::new(base_typography.clone());
    for rule in rules {
        style.apply(rule.style(), base_typography);
    }
    style
}

fn apply_optional<T: Clone>(value: &Specified<T>, target: &mut Option<T>) {
    match value {
        Specified::Unspecified => {}
        Specified::Clear => *target = None,
        Specified::Value(value) => *target = Some(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, OrdinalSelector, ThemeRule,
        ThemeRuleSet,
    };

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
                .for_family(RenderFamilyKind::Flowchart),
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
                .for_family(RenderFamilyKind::State),
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
        let resolved = theme.resolve(RenderFamilyKind::Flowchart);

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
        let resolved = theme.resolve(RenderFamilyKind::Flowchart);

        for variant in ThemeVariant::ALL {
            for ordinal in [None, Some(0), Some(1), Some(2), Some(3)] {
                assert_eq!(
                    resolved.style(ThemeTarget::Node, variant, ordinal),
                    resolve_style(
                        theme.spec(),
                        RenderFamilyKind::Flowchart,
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
}
