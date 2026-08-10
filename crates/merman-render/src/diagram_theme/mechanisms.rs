use std::collections::BTreeSet;

use super::admission::ThemeCapability;
use super::canvas::CanvasPaint;
use super::effects::{EffectGraph, EffectPrimitive};
use super::semantic::ThemeStylePatch;
use super::typography::{Specified, TextStyle, TextStylePatch, TypographySpec};

pub(crate) fn paint_capability(paint: &CanvasPaint) -> Option<ThemeCapability> {
    match paint {
        CanvasPaint::Transparent => Some(ThemeCapability::TransparentPaint),
        CanvasPaint::Solid(_) => Some(ThemeCapability::SolidPaint),
        CanvasPaint::LinearGradient(_) | CanvasPaint::RadialGradient(_) => {
            Some(ThemeCapability::GradientPaint)
        }
        CanvasPaint::Pattern(_) => Some(ThemeCapability::PatternPaint),
    }
}

pub(crate) fn collect_typography_capabilities(
    typography: &TypographySpec,
    required: &mut BTreeSet<ThemeCapability>,
) {
    let library_default = TextStyle::default();
    let theme_default = typography.default_style();
    collect_text_style_capabilities(theme_default, &library_default, required);
    for (_, style) in typography.family_overrides() {
        collect_text_style_capabilities(style, theme_default, required);
    }
}

pub(crate) fn collect_style_patch_capabilities(
    style: &ThemeStylePatch,
    required: &mut BTreeSet<ThemeCapability>,
) {
    if let Specified::Value(paint) = &style.paint.fill
        && let Some(capability) = paint_capability(paint)
    {
        required.insert(capability);
    }
    if let Specified::Value(paint) = &style.stroke.paint
        && let Some(capability) = paint_capability(paint)
    {
        required.insert(capability);
    }
    if !matches!(style.stroke.width, Specified::Unspecified)
        || !matches!(style.stroke.linecap, Specified::Unspecified)
        || !matches!(style.stroke.linejoin, Specified::Unspecified)
    {
        required.insert(ThemeCapability::BorderStyling);
    }
    if !matches!(style.stroke.dasharray, Specified::Unspecified) {
        required.insert(ThemeCapability::DashStyling);
    }
    if !matches!(style.geometry.radius, Specified::Unspecified) {
        required.insert(ThemeCapability::RoundedGeometry);
    }
    collect_text_style_patch_capabilities(&style.typography, required);
    if !matches!(style.spacing.padding, Specified::Unspecified) {
        required.insert(ThemeCapability::ContentPadding);
    }
    if !matches!(style.paint.opacity, Specified::Unspecified)
        || !matches!(style.paint.fill_opacity, Specified::Unspecified)
        || !matches!(style.stroke.stroke_opacity, Specified::Unspecified)
    {
        required.insert(ThemeCapability::Opacity);
    }
}

pub(crate) fn collect_effect_graph_capabilities(
    graph: &EffectGraph,
    required: &mut BTreeSet<ThemeCapability>,
) {
    required.insert(ThemeCapability::SvgFilter);
    for primitive in graph.primitives() {
        match primitive {
            EffectPrimitive::DropShadow { .. } => {
                required.insert(ThemeCapability::Shadow);
            }
            EffectPrimitive::Turbulence { .. } => {
                required.insert(ThemeCapability::Noise);
            }
            EffectPrimitive::Displacement { .. } => {
                required.insert(ThemeCapability::Displacement);
            }
            EffectPrimitive::GaussianBlur { .. } | EffectPrimitive::ColorMatrix { .. } => {}
        }
    }
}

fn collect_text_style_capabilities(
    style: &TextStyle,
    baseline: &TextStyle,
    required: &mut BTreeSet<ThemeCapability>,
) {
    if style != baseline {
        required.insert(ThemeCapability::Typography);
    }
    if style.letter_spacing_px() != baseline.letter_spacing_px() {
        required.insert(ThemeCapability::LetterSpacing);
    }
    if style.word_spacing_px() != baseline.word_spacing_px() {
        required.insert(ThemeCapability::WordSpacing);
    }
    if style.transform() != baseline.transform() {
        required.insert(ThemeCapability::TextTransform);
    }
    if style.decoration() != baseline.decoration() {
        required.insert(ThemeCapability::TextDecoration);
    }
    if style.white_space() != baseline.white_space() || style.wrap() != baseline.wrap() {
        required.insert(ThemeCapability::WhiteSpaceWrapping);
    }
}

fn collect_text_style_patch_capabilities(
    patch: &TextStylePatch,
    required: &mut BTreeSet<ThemeCapability>,
) {
    if !matches!(patch.font_stack, Specified::Unspecified)
        || !matches!(patch.font_size_px, Specified::Unspecified)
        || !matches!(patch.font_weight, Specified::Unspecified)
        || !matches!(patch.font_style, Specified::Unspecified)
        || !matches!(patch.line_height, Specified::Unspecified)
        || !matches!(patch.text_align, Specified::Unspecified)
    {
        required.insert(ThemeCapability::Typography);
    }
    if !matches!(patch.letter_spacing_px, Specified::Unspecified) {
        required.insert(ThemeCapability::Typography);
        required.insert(ThemeCapability::LetterSpacing);
    }
    if !matches!(patch.word_spacing_px, Specified::Unspecified) {
        required.insert(ThemeCapability::Typography);
        required.insert(ThemeCapability::WordSpacing);
    }
    if !matches!(patch.transform, Specified::Unspecified) {
        required.insert(ThemeCapability::Typography);
        required.insert(ThemeCapability::TextTransform);
    }
    if !matches!(patch.decoration, Specified::Unspecified) {
        required.insert(ThemeCapability::Typography);
        required.insert(ThemeCapability::TextDecoration);
    }
    if !matches!(patch.white_space, Specified::Unspecified)
        || !matches!(patch.wrap, Specified::Unspecified)
    {
        required.insert(ThemeCapability::Typography);
        required.insert(ThemeCapability::WhiteSpaceWrapping);
    }
}
