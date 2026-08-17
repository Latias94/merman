use std::collections::BTreeSet;
use std::sync::Arc;

use base64::Engine as _;
use merman_theme_contract as wire;

use crate::DiagramFamilyId;

use super::admission::{
    FontEmbeddingRequirement, FontSource, TextLayoutCapability, ThemeCapability, ThemeRequirements,
};
use super::assets::{
    FontAssetSpec, FontCatalogError, FontCatalogSpec, FontContainer, GenericFontFamily,
    detect_font_container,
};
use super::canvas::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, GradientStop, InsetsPx, LinearGradient,
    PatternKind, PatternSpec, RadialGradient, ThemeColorValue, ThemeLength,
};
use super::effects::{DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive};
use super::semantic::{
    OrdinalPalette, OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use super::spec::{DiagramThemeSpec, MermaidThemeCompatibility, MermaidThemeValue, ThemeAssets};
use super::typography::{
    FontStack, LineHeight, Specified, TextAlign, TextDecoration, TextStyle, TextStylePatch,
    TextTransform, TypographySpec, WhiteSpace, WrapMode,
};
use super::{FontStyle, ThemeCompileError, ThemeCompileValidationError, ThemeResourcePolicy};

pub(super) fn decode(
    spec: wire::DiagramThemeSpecWireV1,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpec, ThemeCompileError> {
    let wire::DiagramThemeSpecWireV1 {
        mermaid,
        typography,
        styles,
        canvas,
        effects,
        requirements,
        assets,
    } = spec;

    let mut decoded = DiagramThemeSpec::new();
    if let Some(mermaid) = mermaid {
        decoded = decoded.with_mermaid_compatibility(decode_mermaid(mermaid)?);
    }
    if let Some(typography) = typography {
        decoded = decoded.with_typography(decode_typography(typography)?);
    }
    if let Some(styles) = styles {
        decoded = decoded.with_styles(decode_styles(styles)?);
    }
    if let Some(canvas) = canvas {
        decoded = decoded.with_canvas(decode_canvas(canvas)?);
    }
    if let Some(effects) = effects {
        decoded = decoded.with_effects(decode_effects(effects)?);
    }
    if let Some(requirements) = requirements {
        decoded = decoded.with_requirements(decode_requirements(requirements)?);
    }
    if let Some(assets) = assets
        && let Some(assets) = decode_assets(assets, resources)?
    {
        decoded = decoded.with_assets(assets);
    }
    Ok(decoded)
}

fn decode_mermaid(
    value: wire::MermaidThemeCompatibilityWireV1,
) -> Result<MermaidThemeCompatibility, ThemeCompileError> {
    let mut decoded = MermaidThemeCompatibility::default();
    if let Some(theme) = value.theme {
        decoded = decoded.with_theme(theme)?;
    }
    if let Some(dark_mode) = value.dark_mode {
        decoded = decoded.with_dark_mode(dark_mode)?;
    }
    for (key, value) in value.variables.unwrap_or_default() {
        let value = match value {
            wire::MermaidThemeValueWireV1::String(value) => MermaidThemeValue::String(value),
            wire::MermaidThemeValueWireV1::Number(value) => MermaidThemeValue::Number(value),
            wire::MermaidThemeValueWireV1::Boolean(value) => MermaidThemeValue::Boolean(value),
        };
        decoded = decoded.with_variable(key, value)?;
    }
    Ok(decoded)
}

fn decode_typography(
    value: wire::ThemeTypographySpecWireV1,
) -> Result<TypographySpec, ThemeCompileError> {
    let mut decoded = TypographySpec::default();
    if let Some(default) = value.default {
        decoded = decoded.with_default(decode_text_style(default)?);
    }
    for (family, style) in value.families.unwrap_or_default() {
        decoded = decoded.with_family_style(
            parse_family(&family, "typography.families")?,
            decode_text_style(style)?,
        );
    }
    Ok(decoded)
}

fn decode_text_style(value: wire::ThemeTextStyleWireV1) -> Result<TextStyle, ThemeCompileError> {
    let mut decoded = TextStyle::default();
    if let Some(font_stack) = value.font_stack {
        decoded = decoded.with_font_stack(FontStack::new(font_stack)?);
    }
    if let Some(font_size_px) = value.font_size_px {
        decoded = decoded.with_font_size_px(font_size_px)?;
    }
    if let Some(font_weight) = value.font_weight {
        decoded = decoded.with_font_weight(font_weight)?;
    }
    if let Some(font_style) = value.font_style {
        decoded = decoded.with_font_style(parse_font_style(&font_style)?);
    }
    if let Some(line_height) = value.line_height {
        decoded = decoded.with_line_height(decode_line_height(line_height)?)?;
    }
    if let Some(letter_spacing_px) = value.letter_spacing_px {
        decoded = decoded.with_letter_spacing_px(letter_spacing_px)?;
    }
    if let Some(word_spacing_px) = value.word_spacing_px {
        decoded = decoded.with_word_spacing_px(word_spacing_px)?;
    }
    if let Some(transform) = value.transform {
        decoded = decoded.with_transform(parse_text_transform(&transform)?);
    }
    if let Some(decoration) = value.decoration {
        decoded = decoded.with_decoration(parse_text_decoration(&decoration)?);
    }
    if let Some(text_align) = value.text_align {
        decoded = decoded.with_text_align(parse_text_align(&text_align)?);
    }
    if let Some(white_space) = value.white_space {
        decoded = decoded.with_white_space(parse_white_space(&white_space)?);
    }
    if let Some(wrap) = value.wrap {
        decoded = decoded.with_wrap(parse_wrap_mode(&wrap)?);
    }
    Ok(decoded)
}

fn decode_styles(
    entries: Vec<wire::ThemeRuleSetWireV1>,
) -> Result<ThemeRuleSet, ThemeCompileError> {
    let mut decoded = ThemeRuleSet::default();
    for entry in entries {
        match entry {
            wire::ThemeRuleSetWireV1::Rule {
                target,
                family,
                variant,
                ordinal,
                style,
            } => {
                let mut rule = ThemeRule::new(
                    parse_target(&target, "styles.rule.target")?,
                    decode_style_patch(style)?,
                );
                if let Some(family) = family {
                    rule = rule.for_family(parse_family(&family, "styles.rule.family")?);
                }
                if let Some(variant) = variant {
                    rule = rule.with_variant(parse_variant(&variant, "styles.rule.variant")?);
                }
                if let Some(ordinal) = ordinal {
                    rule = rule.with_ordinal(decode_ordinal(ordinal)?);
                }
                decoded = decoded.with_rule(rule);
            }
            wire::ThemeRuleSetWireV1::OrdinalPalette { target, colors } => {
                let colors = colors
                    .into_iter()
                    .map(|color| ThemeColorValue::parse(&color))
                    .collect::<Result<Vec<_>, _>>()?;
                decoded = decoded.with_ordinal_palette(
                    parse_target(&target, "styles.ordinal_palette.target")?,
                    OrdinalPalette::new(colors)?,
                );
            }
        }
    }
    Ok(decoded)
}

fn decode_ordinal(
    value: wire::ThemeOrdinalSelectorWireV1,
) -> Result<OrdinalSelector, ThemeCompileError> {
    match value {
        wire::ThemeOrdinalSelectorWireV1::Exact { exact } => Ok(OrdinalSelector::exact(
            usize::try_from(exact).map_err(|_| ThemeCompileValidationError::InvalidNumber {
                field: "styles.rule.ordinal.exact",
            })?,
        )?),
        wire::ThemeOrdinalSelectorWireV1::Cycle { cycle } => Ok(OrdinalSelector::cycle(
            usize::try_from(cycle.period).map_err(|_| {
                ThemeCompileValidationError::InvalidNumber {
                    field: "styles.rule.ordinal.cycle",
                }
            })?,
            usize::try_from(cycle.offset).map_err(|_| {
                ThemeCompileValidationError::InvalidNumber {
                    field: "styles.rule.ordinal.cycle",
                }
            })?,
        )?),
    }
}

fn decode_style_patch(
    value: wire::ThemeStylePatchWireV1,
) -> Result<ThemeStylePatch, ThemeCompileError> {
    let mut decoded = ThemeStylePatch::default();
    decoded.paint.fill = decode_specified(value.fill, decode_paint)?;
    decoded.paint.opacity = decode_specified_value(value.opacity);
    decoded.paint.fill_opacity = decode_specified_value(value.fill_opacity);
    decoded.stroke = match value.stroke {
        wire::SpecifiedWireV1::Unspecified => Default::default(),
        wire::SpecifiedWireV1::Clear => cleared_stroke_patch(),
        wire::SpecifiedWireV1::Value(stroke) => decode_stroke_patch(stroke)?,
    };
    decoded.geometry.radius = decode_specified_value(value.radius);
    decoded.spacing.padding = decode_specified(value.padding, |value| Ok(decode_insets(value)))?;
    decoded.typography = match value.typography {
        wire::SpecifiedWireV1::Unspecified => Default::default(),
        wire::SpecifiedWireV1::Clear => cleared_text_style_patch(),
        wire::SpecifiedWireV1::Value(typography) => decode_text_style_patch(typography)?,
    };
    decoded.effects.effect = decode_specified_value(value.effect);
    Ok(decoded)
}

fn decode_stroke_patch(
    value: wire::ThemeStrokePatchWireV1,
) -> Result<super::ThemeStrokePatch, ThemeCompileError> {
    Ok(super::ThemeStrokePatch {
        paint: decode_specified(value.paint, decode_paint)?,
        width: decode_specified_value(value.width),
        dasharray: decode_specified_value(value.dasharray),
        linecap: decode_specified(value.linecap, |value| parse_stroke_line_cap(&value))?,
        linejoin: decode_specified(value.linejoin, |value| parse_stroke_line_join(&value))?,
        stroke_opacity: decode_specified_value(value.opacity),
    })
}

fn decode_text_style_patch(
    value: wire::ThemeTextStylePatchWireV1,
) -> Result<TextStylePatch, ThemeCompileError> {
    Ok(TextStylePatch {
        font_stack: decode_specified(value.font_stack, |families| Ok(FontStack::new(families)?))?,
        font_size_px: decode_specified_value(value.font_size_px),
        font_weight: decode_specified_value(value.font_weight),
        font_style: decode_specified(value.font_style, |value| parse_font_style(&value))?,
        line_height: decode_specified(value.line_height, decode_line_height)?,
        letter_spacing_px: decode_specified_value(value.letter_spacing_px),
        word_spacing_px: decode_specified_value(value.word_spacing_px),
        transform: decode_specified(value.transform, |value| parse_text_transform(&value))?,
        decoration: decode_specified(value.decoration, |value| parse_text_decoration(&value))?,
        text_align: decode_specified(value.text_align, |value| parse_text_align(&value))?,
        white_space: decode_specified(value.white_space, |value| parse_white_space(&value))?,
        wrap: decode_specified(value.wrap, |value| parse_wrap_mode(&value))?,
    })
}

fn cleared_stroke_patch() -> super::ThemeStrokePatch {
    super::ThemeStrokePatch {
        paint: Specified::Clear,
        width: Specified::Clear,
        dasharray: Specified::Clear,
        linecap: Specified::Clear,
        linejoin: Specified::Clear,
        stroke_opacity: Specified::Clear,
    }
}

fn cleared_text_style_patch() -> TextStylePatch {
    TextStylePatch {
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
    }
}

fn decode_canvas(value: wire::ThemeCanvasSpecWireV1) -> Result<CanvasSpec, ThemeCompileError> {
    let mut decoded = CanvasSpec::default();
    if let Some(base) = value.base {
        decoded = decoded.with_base(decode_paint(base)?);
    }
    for layer in value.layers.unwrap_or_default() {
        decoded = decoded.with_layer(decode_canvas_layer(layer)?)?;
    }
    if let Some(bleed) = value.bleed {
        decoded = decoded.with_bleed(decode_insets(bleed))?;
    }
    Ok(decoded)
}

fn decode_canvas_layer(
    value: wire::ThemeCanvasLayerWireV1,
) -> Result<CanvasLayer, ThemeCompileError> {
    let mut decoded = CanvasLayer::new(decode_paint(value.paint)?);
    if let Some(opacity) = value.opacity {
        decoded = decoded.with_opacity(opacity)?;
    }
    if let Some(blend_mode) = value.blend_mode {
        decoded = decoded.with_blend_mode(parse_blend_mode(&blend_mode)?);
    }
    if value.offset_x.is_some() || value.offset_y.is_some() {
        decoded =
            decoded.with_offset(value.offset_x.unwrap_or(0.0), value.offset_y.unwrap_or(0.0))?;
    }
    Ok(decoded)
}

fn decode_paint(value: wire::ThemeCanvasPaintWireV1) -> Result<CanvasPaint, ThemeCompileError> {
    match value {
        wire::ThemeCanvasPaintWireV1::Color(value) if value.trim() == "transparent" => {
            Ok(CanvasPaint::Transparent)
        }
        wire::ThemeCanvasPaintWireV1::Color(value) => Ok(CanvasPaint::solid(value)?),
        wire::ThemeCanvasPaintWireV1::Structured(value) => decode_structured_paint(value),
    }
}

fn decode_structured_paint(
    value: wire::ThemeCanvasPaintObjectWireV1,
) -> Result<CanvasPaint, ThemeCompileError> {
    match value {
        wire::ThemeCanvasPaintObjectWireV1::Transparent => Ok(CanvasPaint::Transparent),
        wire::ThemeCanvasPaintObjectWireV1::Solid { color } => Ok(CanvasPaint::solid(color)?),
        wire::ThemeCanvasPaintObjectWireV1::LinearGradient {
            angle_degrees,
            stops,
            repetition,
        } => {
            let mut gradient = LinearGradient::new(angle_degrees, decode_gradient_stops(stops)?)?;
            if let Some(repetition) = repetition {
                gradient = match repetition {
                    wire::ThemeLinearGradientRepetitionWireV1::Repeating { period_px } => {
                        gradient.with_repeating_period_px(period_px)?
                    }
                    wire::ThemeLinearGradientRepetitionWireV1::Tiled {
                        width_px,
                        height_px,
                    } => gradient.with_tile_px(width_px, height_px)?,
                };
            }
            Ok(CanvasPaint::LinearGradient(gradient))
        }
        wire::ThemeCanvasPaintObjectWireV1::RadialGradient {
            center_x,
            center_y,
            radius,
            stops,
            repetition,
        } => {
            let mut gradient = RadialGradient::new(
                decode_length(center_x),
                decode_length(center_y),
                decode_length(radius),
                decode_gradient_stops(stops)?,
            )?;
            if let Some(repetition) = repetition {
                gradient = match repetition {
                    wire::ThemeRadialGradientRepetitionWireV1::Repeating => {
                        gradient.with_repeating()?
                    }
                    wire::ThemeRadialGradientRepetitionWireV1::Tiled {
                        width_px,
                        height_px,
                    } => gradient.with_tile_px(width_px, height_px)?,
                };
            }
            Ok(CanvasPaint::RadialGradient(gradient))
        }
        wire::ThemeCanvasPaintObjectWireV1::Pattern {
            pattern,
            cell_width,
            cell_height,
            foreground,
            background,
            angle_degrees,
        } => {
            let mut pattern = PatternSpec::new(
                parse_pattern_kind(&pattern)?,
                cell_width,
                cell_height,
                ThemeColorValue::parse(&foreground)?,
            )?;
            if let Some(background) = background {
                pattern = pattern.with_background(ThemeColorValue::parse(&background)?);
            }
            if let Some(angle_degrees) = angle_degrees {
                pattern = pattern.with_angle_degrees(angle_degrees)?;
            }
            Ok(CanvasPaint::Pattern(pattern))
        }
    }
}

fn decode_gradient_stops(
    stops: Vec<wire::ThemeGradientStopWireV1>,
) -> Result<Vec<GradientStop>, ThemeCompileError> {
    stops
        .into_iter()
        .map(|stop| {
            Ok(GradientStop::new(
                stop.offset,
                ThemeColorValue::parse(&stop.color)?,
            )?)
        })
        .collect()
}

const fn decode_length(value: wire::ThemeLengthWireV1) -> ThemeLength {
    match value {
        wire::ThemeLengthWireV1::PxValue(value) => ThemeLength::px(value),
        wire::ThemeLengthWireV1::Px { px } => ThemeLength::px(px),
        wire::ThemeLengthWireV1::Percent { percent } => ThemeLength::percent(percent),
    }
}

const fn decode_insets(value: wire::ThemeInsetsWireV1) -> InsetsPx {
    match value {
        wire::ThemeInsetsWireV1::All(value) => InsetsPx::all(value),
        wire::ThemeInsetsWireV1::Sides {
            top,
            right,
            bottom,
            left,
        } => InsetsPx {
            top,
            right,
            bottom,
            left,
        },
    }
}

fn decode_effects(
    entries: Vec<wire::ThemeEffectEntryWireV1>,
) -> Result<DiagramEffectSet, ThemeCompileError> {
    let mut decoded = DiagramEffectSet::default();
    for entry in entries {
        match entry {
            wire::ThemeEffectEntryWireV1::Graph { id, primitives } => {
                let primitives = primitives
                    .into_iter()
                    .map(decode_effect_primitive)
                    .collect::<Result<Vec<_>, _>>()?;
                decoded = decoded.with_graph(EffectGraph::new(id, primitives)?)?;
            }
            wire::ThemeEffectEntryWireV1::Binding { target, effect_id } => {
                decoded = decoded.with_binding(EffectBinding::new(
                    parse_target(&target, "effects.binding.target")?,
                    effect_id,
                )?)?;
            }
        }
    }
    Ok(decoded)
}

fn decode_effect_primitive(
    value: wire::ThemeEffectPrimitiveWireV1,
) -> Result<EffectPrimitive, ThemeCompileError> {
    match value {
        wire::ThemeEffectPrimitiveWireV1::DropShadow {
            input,
            offset_x,
            offset_y,
            blur_radius,
            spread,
            color,
        } => Ok(EffectPrimitive::DropShadow {
            input: decode_optional_effect_input(input.as_deref())?,
            offset_x,
            offset_y,
            blur_radius,
            spread,
            color: ThemeColorValue::parse(&color)?,
        }),
        wire::ThemeEffectPrimitiveWireV1::GaussianBlur {
            input,
            std_deviation,
        } => Ok(EffectPrimitive::GaussianBlur {
            input: decode_optional_effect_input(input.as_deref())?,
            std_deviation,
        }),
        wire::ThemeEffectPrimitiveWireV1::ColorMatrix { input, values } => {
            let values = <[f32; 20]>::try_from(values).map_err(|_| {
                ThemeCompileValidationError::InvalidCollection {
                    field: "effects.color_matrix.values",
                }
            })?;
            Ok(EffectPrimitive::ColorMatrix {
                input: decode_optional_effect_input(input.as_deref())?,
                values,
            })
        }
        wire::ThemeEffectPrimitiveWireV1::Turbulence {
            input,
            base_frequency_x,
            base_frequency_y,
            octaves,
            seed,
        } => Ok(EffectPrimitive::Turbulence {
            input: decode_optional_effect_input(input.as_deref())?,
            base_frequency_x,
            base_frequency_y,
            octaves,
            seed,
        }),
        wire::ThemeEffectPrimitiveWireV1::Displacement {
            input,
            map_input,
            scale,
        } => Ok(EffectPrimitive::Displacement {
            input: decode_optional_effect_input(input.as_deref())?,
            map_input: parse_effect_input(&map_input)?,
            scale,
        }),
    }
}

fn decode_requirements(
    value: wire::ThemeRequirementsWireV1,
) -> Result<ThemeRequirements, ThemeCompileError> {
    let capabilities = value
        .capabilities
        .unwrap_or_default()
        .into_iter()
        .map(|value| {
            parse_id(
                &value,
                ThemeCapability::ALL,
                ThemeCapability::id,
                "requirements.capabilities",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let text_capabilities = value
        .text_capabilities
        .unwrap_or_default()
        .into_iter()
        .map(|value| {
            parse_id(
                &value,
                TextLayoutCapability::ALL,
                TextLayoutCapability::id,
                "requirements.text_capabilities",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ThemeRequirements::new()
        .with_required_capabilities(capabilities)
        .with_required_text_capabilities(text_capabilities))
}

fn decode_assets(
    value: wire::ThemeAssetsWireV1,
    resources: &ThemeResourcePolicy,
) -> Result<Option<ThemeAssets>, ThemeCompileError> {
    let fonts = value.fonts.unwrap_or_default();
    let aliases = value.aliases.unwrap_or_default();
    let generic_families = value.generic_families.unwrap_or_default();

    if fonts.is_empty() {
        if aliases.is_empty()
            && generic_families.is_empty()
            && value.available_sources.is_none()
            && value.embedding.is_none()
        {
            return Ok(None);
        }
        return Err(ThemeCompileValidationError::InvalidCollection {
            field: "assets.fonts",
        }
        .into());
    }

    resources.check_font_asset_count(fonts.len())?;
    let mapping_count = aliases
        .len()
        .checked_add(generic_families.len())
        .ok_or(FontCatalogError::CatalogCountOverflow)?;
    resources.check_font_alias_count(mapping_count)?;

    let mut ids = BTreeSet::new();
    for font in &fonts {
        if !ids.insert(font.id.as_str()) {
            return Err(FontCatalogError::DuplicateAssetId {
                id: font.id.clone(),
            }
            .into());
        }
    }

    let base64_bytes = fonts.iter().try_fold(0usize, |total, font| {
        total.checked_add(font.data_base64.len())
    });
    resources.check_theme_base64_bytes(
        base64_bytes.ok_or(FontCatalogError::CatalogByteCountOverflow)?,
    )?;

    let available_sources = value
        .available_sources
        .map(|sources| {
            sources
                .into_iter()
                .map(|source| {
                    parse_id(
                        &source,
                        FontSource::ALL,
                        FontSource::id,
                        "assets.available_sources",
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let embedding = value
        .embedding
        .map(|value| parse_embedding_requirement(&value))
        .transpose()?;
    let generic_families = generic_families
        .into_iter()
        .map(|mapping| {
            Ok((
                parse_id(
                    &mapping.generic,
                    GenericFontFamily::ALL,
                    GenericFontFamily::id,
                    "assets.generic_families.generic",
                )?,
                mapping.target,
            ))
        })
        .collect::<Result<Vec<_>, ThemeCompileError>>()?;
    let declared_formats = fonts
        .iter()
        .map(|font| {
            parse_id(
                &font.format,
                FontContainer::ALL,
                FontContainer::id,
                "assets.fonts.format",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut font_specs = Vec::with_capacity(fonts.len());
    for (asset_index, (font, declared_format)) in
        fonts.into_iter().zip(declared_formats).enumerate()
    {
        let bytes = decode_canonical_base64(&font.data_base64)?;
        let actual_format = detect_font_container(&bytes)
            .ok_or(FontCatalogError::UnsupportedFontContainer { asset_index })?;
        if actual_format != declared_format {
            return Err(ThemeCompileValidationError::InvalidValue {
                field: "assets.fonts.format",
            }
            .into());
        }
        font_specs.push(FontAssetSpec::from_shared_bytes(
            font.id,
            Arc::<[u8]>::from(bytes),
        ));
    }

    let mut catalog = FontCatalogSpec::new(font_specs);
    for alias in aliases {
        catalog = catalog.with_alias(alias.alias, alias.target);
    }
    for (generic, target) in generic_families {
        catalog = catalog.with_generic_family(generic, target);
    }
    if let Some(available_sources) = available_sources {
        catalog = catalog.with_available_sources(available_sources);
    }
    if let Some(embedding) = embedding {
        catalog = catalog.with_embedding_requirement(embedding);
    }
    Ok(Some(ThemeAssets::default().with_font_catalog(catalog)))
}

fn decode_canonical_base64(value: &str) -> Result<Vec<u8>, ThemeCompileError> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| {
            ThemeCompileValidationError::InvalidValue {
                field: "assets.fonts.data_base64",
            }
            .into()
        })
}

fn decode_line_height(value: wire::ThemeLineHeightWireV1) -> Result<LineHeight, ThemeCompileError> {
    match value {
        wire::ThemeLineHeightWireV1::Keyword(value) if value == "normal" => Ok(LineHeight::Normal),
        wire::ThemeLineHeightWireV1::Keyword(_) => Err(ThemeCompileValidationError::InvalidValue {
            field: "typography.line_height",
        }
        .into()),
        wire::ThemeLineHeightWireV1::Multiplier(value) => Ok(LineHeight::Multiplier(value)),
        wire::ThemeLineHeightWireV1::Px { px } => Ok(LineHeight::Px(px)),
    }
}

fn parse_family(
    value: &str,
    field: &'static str,
) -> Result<DiagramFamilyId, ThemeCompileValidationError> {
    DiagramFamilyId::from_id(value).ok_or(ThemeCompileValidationError::UnknownId { field })
}

fn parse_target(
    value: &str,
    field: &'static str,
) -> Result<ThemeTarget, ThemeCompileValidationError> {
    parse_id(value, ThemeTarget::ALL, ThemeTarget::id, field)
}

fn parse_variant(
    value: &str,
    field: &'static str,
) -> Result<ThemeVariant, ThemeCompileValidationError> {
    parse_id(value, ThemeVariant::ALL, ThemeVariant::id, field)
}

fn parse_font_style(value: &str) -> Result<FontStyle, ThemeCompileError> {
    Ok(parse_id(
        value,
        FontStyle::ALL,
        FontStyle::id,
        "typography.font_style",
    )?)
}

fn parse_text_transform(value: &str) -> Result<TextTransform, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", TextTransform::None),
            ("uppercase", TextTransform::Uppercase),
            ("lowercase", TextTransform::Lowercase),
            ("capitalize", TextTransform::Capitalize),
        ],
        "typography.transform",
    )
}

fn parse_text_decoration(value: &str) -> Result<TextDecoration, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", TextDecoration::None),
            ("underline", TextDecoration::Underline),
            ("overline", TextDecoration::Overline),
            ("line-through", TextDecoration::LineThrough),
        ],
        "typography.decoration",
    )
}

fn parse_text_align(value: &str) -> Result<TextAlign, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("start", TextAlign::Start),
            ("center", TextAlign::Center),
            ("end", TextAlign::End),
        ],
        "typography.text_align",
    )
}

fn parse_white_space(value: &str) -> Result<WhiteSpace, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", WhiteSpace::Normal),
            ("pre", WhiteSpace::Pre),
            ("no-wrap", WhiteSpace::NoWrap),
            ("pre-wrap", WhiteSpace::PreWrap),
            ("pre-line", WhiteSpace::PreLine),
        ],
        "typography.white_space",
    )
}

fn parse_wrap_mode(value: &str) -> Result<WrapMode, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", WrapMode::Normal),
            ("break-word", WrapMode::BreakWord),
            ("anywhere", WrapMode::Anywhere),
        ],
        "typography.wrap",
    )
}

fn parse_stroke_line_cap(value: &str) -> Result<StrokeLineCap, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("butt", StrokeLineCap::Butt),
            ("round", StrokeLineCap::Round),
            ("square", StrokeLineCap::Square),
        ],
        "styles.rule.stroke.linecap",
    )
}

fn parse_stroke_line_join(value: &str) -> Result<StrokeLineJoin, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("miter", StrokeLineJoin::Miter),
            ("round", StrokeLineJoin::Round),
            ("bevel", StrokeLineJoin::Bevel),
        ],
        "styles.rule.stroke.linejoin",
    )
}

fn parse_pattern_kind(value: &str) -> Result<PatternKind, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("dots", PatternKind::Dots),
            ("grid", PatternKind::Grid),
            ("stripes", PatternKind::Stripes),
        ],
        "canvas.pattern.kind",
    )
}

fn parse_blend_mode(value: &str) -> Result<BlendMode, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", BlendMode::Normal),
            ("multiply", BlendMode::Multiply),
            ("screen", BlendMode::Screen),
            ("overlay", BlendMode::Overlay),
            ("darken", BlendMode::Darken),
            ("lighten", BlendMode::Lighten),
            ("difference", BlendMode::Difference),
        ],
        "canvas.layers.blend_mode",
    )
}

fn parse_effect_input(value: &str) -> Result<EffectInput, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("source-graphic", EffectInput::SourceGraphic),
            ("previous", EffectInput::Previous),
        ],
        "effects.input",
    )
}

fn decode_optional_effect_input(value: Option<&str>) -> Result<EffectInput, ThemeCompileError> {
    value.map_or(Ok(EffectInput::SourceGraphic), parse_effect_input)
}

fn parse_embedding_requirement(value: &str) -> Result<FontEmbeddingRequirement, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", FontEmbeddingRequirement::NoEmbedding),
            ("full-font", FontEmbeddingRequirement::FullFont),
            ("subset", FontEmbeddingRequirement::Subset),
        ],
        "assets.embedding",
    )
}

fn parse_id<T: Copy>(
    value: &str,
    values: &[T],
    id: impl Fn(T) -> &'static str,
    field: &'static str,
) -> Result<T, ThemeCompileValidationError> {
    values
        .iter()
        .copied()
        .find(|candidate| id(*candidate) == value)
        .ok_or(ThemeCompileValidationError::UnknownId { field })
}

fn parse_small_id<T: Copy>(
    value: &str,
    values: &[(&'static str, T)],
    field: &'static str,
) -> Result<T, ThemeCompileError> {
    values
        .iter()
        .find_map(|(id, candidate)| (*id == value).then_some(*candidate))
        .ok_or_else(|| ThemeCompileValidationError::UnknownId { field }.into())
}

fn decode_specified<T, U>(
    value: wire::SpecifiedWireV1<T>,
    decode: impl FnOnce(T) -> Result<U, ThemeCompileError>,
) -> Result<Specified<U>, ThemeCompileError> {
    match value {
        wire::SpecifiedWireV1::Unspecified => Ok(Specified::Unspecified),
        wire::SpecifiedWireV1::Clear => Ok(Specified::Clear),
        wire::SpecifiedWireV1::Value(value) => decode(value).map(Specified::Value),
    }
}

fn decode_specified_value<T>(value: wire::SpecifiedWireV1<T>) -> Specified<T> {
    match value {
        wire::SpecifiedWireV1::Unspecified => Specified::Unspecified,
        wire::SpecifiedWireV1::Clear => Specified::Clear,
        wire::SpecifiedWireV1::Value(value) => Specified::Value(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroke_and_typography_group_clear_lower_atomically() {
        let spec = wire::DiagramThemeSpecWireV1 {
            styles: Some(vec![wire::ThemeRuleSetWireV1::Rule {
                target: "node".to_string(),
                family: None,
                variant: None,
                ordinal: None,
                style: wire::ThemeStylePatchWireV1 {
                    stroke: wire::SpecifiedWireV1::Clear,
                    typography: wire::SpecifiedWireV1::Clear,
                    ..Default::default()
                },
            }]),
            ..Default::default()
        };

        let decoded = decode(spec, &ThemeResourcePolicy::default()).expect("decode complete spec");
        let style = decoded.styles().rules()[0].style();
        assert!(matches!(style.stroke.paint, Specified::Clear));
        assert!(matches!(style.stroke.width, Specified::Clear));
        assert!(matches!(style.stroke.dasharray, Specified::Clear));
        assert!(matches!(style.stroke.linecap, Specified::Clear));
        assert!(matches!(style.stroke.linejoin, Specified::Clear));
        assert!(matches!(style.stroke.stroke_opacity, Specified::Clear));
        assert!(matches!(style.typography.font_stack, Specified::Clear));
        assert!(matches!(style.typography.font_size_px, Specified::Clear));
        assert!(matches!(style.typography.font_weight, Specified::Clear));
        assert!(matches!(style.typography.font_style, Specified::Clear));
        assert!(matches!(style.typography.line_height, Specified::Clear));
        assert!(matches!(
            style.typography.letter_spacing_px,
            Specified::Clear
        ));
        assert!(matches!(style.typography.word_spacing_px, Specified::Clear));
        assert!(matches!(style.typography.transform, Specified::Clear));
        assert!(matches!(style.typography.decoration, Specified::Clear));
        assert!(matches!(style.typography.text_align, Specified::Clear));
        assert!(matches!(style.typography.white_space, Specified::Clear));
        assert!(matches!(style.typography.wrap, Specified::Clear));
    }
}
