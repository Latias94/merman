use crate::family::RenderFamilyKind;

use super::canvas::{CanvasPaint, InsetsPx, ThemeColorValue};
use super::compiler::ThemeCapabilityReport;
use super::semantic::{StrokeLineCap, StrokeLineJoin, ThemeStylePatch, ThemeTarget, ThemeVariant};
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
}

impl ResolvedDiagramTheme {
    pub(crate) fn new(theme: super::DiagramTheme, family: RenderFamilyKind) -> Self {
        Self { theme, family }
    }

    pub const fn family(&self) -> RenderFamilyKind {
        self.family
    }

    pub const fn theme(&self) -> &super::DiagramTheme {
        &self.theme
    }

    pub fn typography(&self) -> &TextStyle {
        self.theme.spec().typography().family_style(self.family)
    }

    pub fn style(
        &self,
        target: ThemeTarget,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> ResolvedThemeStyle {
        resolve_style(self.theme.spec(), self.family, target, variant, ordinal)
    }

    pub fn series_color(
        &self,
        target: ThemeTarget,
        one_based_index: usize,
    ) -> Option<&ThemeColorValue> {
        if !target.valid_for(self.family) {
            return None;
        }
        self.theme
            .spec()
            .styles()
            .ordinal_palettes()
            .iter()
            .find(|(palette_target, _)| *palette_target == target)
            .and_then(|(_, palette)| palette.color_for(one_based_index))
    }

    pub fn capabilities(&self) -> &ThemeCapabilityReport {
        self.theme.capabilities()
    }

    pub fn into_theme(self) -> super::DiagramTheme {
        self.theme
    }
}

pub(crate) fn resolve_style(
    spec: &super::DiagramThemeSpec,
    family: RenderFamilyKind,
    target: ThemeTarget,
    variant: ThemeVariant,
    ordinal: Option<usize>,
) -> ResolvedThemeStyle {
    let base_typography = spec.typography().family_style(family).clone();
    let mut style = ResolvedThemeStyle::new(base_typography.clone());
    for rule in spec
        .styles()
        .matching_rules(family, target, variant, ordinal)
    {
        style.apply(rule.style(), &base_typography);
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
