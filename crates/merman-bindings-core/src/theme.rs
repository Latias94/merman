use std::collections::BTreeMap;

use base64::Engine as _;
use merman::MermaidThemeId;
use merman::svg::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme,
    DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph, EffectInput,
    EffectPrimitive, FilterRegion, FontAssetSpec, FontCatalogSpec, FontContainer,
    FontEmbeddingRequirement, FontSource, FontStack, FontStyle, GenericFontFamily, GradientStop,
    InsetsPx, LineHeight, LinearGradient, MermaidThemeCompatibility, MermaidThemeValue,
    OrdinalPalette, OrdinalSelector, PatternKind, PatternSpec, RadialGradient, RenderFamilyKind,
    Specified, StrokeLineCap, StrokeLineJoin, TextAlign, TextDecoration, TextLayoutCapability,
    TextStylePatch, TextTransform, ThemeAssets, ThemeCapability, ThemeColorValue, ThemeLength,
    ThemeRequirements, ThemeResourcePolicy, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, ThemeVariant, ThemeWrapMode, TypographySpec, WhiteSpace,
};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, value::RawValue};

use crate::common::{BindingError, BindingStatus};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingThemeOptionsJson {
    preset: Option<String>,
    spec: Option<BindingDiagramThemeSpecJson>,
}

#[derive(Debug, Deserialize)]
struct BindingThemeInputProbe<'a> {
    #[serde(borrow)]
    theme: Option<&'a RawValue>,
}

/// Checks the exact encoded theme slice before Serde allocates the typed theme graph.
#[cfg(test)]
pub(crate) fn validate_theme_input_json(options_json: &[u8]) -> Result<(), BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    validate_theme_input_json_with(&compiler, options_json)
}

/// Checks the exact encoded theme slice against a caller-owned host ceiling.
pub(crate) fn validate_theme_input_json_with(
    compiler: &DiagramThemeCompiler,
    options_json: &[u8],
) -> Result<(), BindingError> {
    if options_json.is_empty() {
        return Ok(());
    }
    let probe: BindingThemeInputProbe<'_> =
        serde_json::from_slice(options_json).map_err(|error| {
            BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid options_json: {error}"),
            )
        })?;
    let Some(theme) = probe.theme else {
        return Ok(());
    };
    compiler
        .check_encoded_input_bytes(theme.get().len())
        .map_err(theme_resource_error)
}

/// Compiles one exact `{"preset": ...}` or `{"spec": ...}` selection with default host policy.
pub fn compile_theme_selection_json(bytes: &[u8]) -> Result<DiagramTheme, BindingError> {
    let compiler =
        DiagramThemeCompiler::new().with_resource_policy(ThemeResourcePolicy::for_profile(
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
        ));
    compile_theme_selection_json_with(&compiler, bytes)
}

/// Compiles one exact theme selection with a caller-owned compiler policy.
pub fn compile_theme_selection_json_with(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, BindingError> {
    compiler
        .check_encoded_input_bytes(bytes.len())
        .map_err(theme_resource_error)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid theme selection JSON: {error}"),
        )
    })?;
    validate_theme_wire(Some(&value))?;
    let selection = serde_json::from_value::<BindingThemeOptionsJson>(value).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid theme selection JSON: {error}"),
        )
    })?;
    compile_theme_with(compiler, Some(&selection))?.ok_or_else(|| {
        BindingError::internal("validated theme selection did not produce a compiled theme")
    })
}

/// Validates the parts of the theme contract that must remain visible in the original JSON.
///
/// In particular, Serde's `Option` intentionally maps both an omitted field and JSON `null` to
/// `None`. The outer options overlay needs that behavior for `theme: null`, but the tagged union
/// itself must still reject `{}`, a null payload, or simultaneous `preset` and `spec` members.
pub(crate) fn validate_theme_wire(theme: Option<&Value>) -> Result<(), BindingError> {
    let Some(theme) = theme else {
        return Ok(());
    };
    if theme.is_null() {
        return Ok(());
    }
    let object = theme
        .as_object()
        .ok_or_else(|| invalid_options("options field `theme` must be an object or null"))?;
    let has_preset = object.contains_key("preset");
    let has_spec = object.contains_key("spec");
    match (has_preset, has_spec) {
        (true, false) | (false, true) => {}
        (false, false) => {
            return Err(invalid_options(
                "options field `theme` must contain exactly one of `preset` or `spec`",
            ));
        }
        (true, true) => {
            return Err(invalid_options(
                "options field `theme` must not contain both `preset` and `spec`",
            ));
        }
    }
    if object
        .get(if has_preset { "preset" } else { "spec" })
        .is_some_and(Value::is_null)
    {
        return Err(invalid_options(
            "options field `theme.preset` or `theme.spec` must not be null",
        ));
    }

    Ok(())
}

pub(crate) fn compile_theme_with(
    compiler: &DiagramThemeCompiler,
    selection: Option<&BindingThemeOptionsJson>,
) -> Result<Option<DiagramTheme>, BindingError> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let theme = match (&selection.preset, &selection.spec) {
        (Some(id), None) => {
            let preset = merman::svg::ThemePreset::from_id(id.trim()).map_err(|_| {
                invalid_theme("theme.preset", format!("unknown theme preset `{id}`"))
            })?;
            compiler
                .compile_preset(preset)
                .map_err(theme_compile_error)?
        }
        (None, Some(spec)) => {
            let expected_formats = spec.expected_font_formats()?;
            let theme = compiler
                .compile(spec.to_theme_spec(compiler.resource_policy())?)
                .map_err(theme_compile_error)?;
            validate_compiled_font_formats(&theme, &expected_formats)?;
            theme
        }
        _ => {
            return Err(invalid_options(
                "options field `theme` must contain exactly one of `preset` or `spec`",
            ));
        }
    };
    Ok(Some(theme))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingDiagramThemeSpecJson {
    mermaid: Option<BindingMermaidCompatibilityJson>,
    typography: Option<BindingTypographySpecJson>,
    #[serde(default)]
    styles: Vec<BindingStyleEntryJson>,
    canvas: Option<BindingCanvasSpecJson>,
    #[serde(default)]
    effects: Vec<BindingEffectEntryJson>,
    requirements: Option<BindingThemeRequirementsJson>,
    assets: Option<BindingThemeAssetsJson>,
}

impl BindingDiagramThemeSpecJson {
    fn to_theme_spec(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<DiagramThemeSpec, BindingError> {
        let mut spec = DiagramThemeSpec::new();
        if let Some(mermaid) = &self.mermaid {
            spec = spec.with_mermaid_compatibility(mermaid.to_compatibility()?);
        }
        if let Some(typography) = &self.typography {
            spec = spec.with_typography(typography.to_typography()?);
        }
        spec = spec.with_styles(binding_styles(&self.styles)?);
        if let Some(canvas) = &self.canvas {
            spec = spec.with_canvas(canvas.to_canvas()?);
        }
        spec = spec.with_effects(binding_effects(&self.effects)?);
        if let Some(requirements) = &self.requirements {
            spec = spec.with_requirements(requirements.to_requirements()?);
        }
        if let Some(assets) = &self.assets
            && let Some(catalog) = assets.to_font_catalog(resources)?
        {
            spec = spec.with_assets(ThemeAssets::default().with_font_catalog(catalog));
        }
        Ok(spec)
    }

    fn expected_font_formats(&self) -> Result<BTreeMap<String, FontContainer>, BindingError> {
        let Some(assets) = &self.assets else {
            return Ok(BTreeMap::new());
        };
        assets.expected_font_formats()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingMermaidCompatibilityJson {
    theme: Option<String>,
    dark_mode: Option<bool>,
    #[serde(default)]
    variables: BTreeMap<String, Value>,
}

impl BindingMermaidCompatibilityJson {
    fn to_compatibility(&self) -> Result<MermaidThemeCompatibility, BindingError> {
        let mut compatibility = MermaidThemeCompatibility::default();
        if let Some(theme) = self.theme.as_deref() {
            let theme = theme.trim();
            let theme_id = MermaidThemeId::parse(theme)
                .map_err(|error| invalid_theme("theme.spec.mermaid.theme", error.to_string()))?;
            compatibility = compatibility.with_theme_id(theme_id);
        }
        if let Some(dark_mode) = self.dark_mode {
            compatibility = compatibility
                .with_dark_mode(dark_mode)
                .map_err(|error| theme_value_error("theme.spec.mermaid.dark_mode", error))?;
        }
        for (key, value) in &self.variables {
            let value =
                match value {
                    Value::String(value) => MermaidThemeValue::String(value.clone()),
                    Value::Number(value) => value
                        .as_f64()
                        .map(MermaidThemeValue::Number)
                        .ok_or_else(|| {
                            invalid_theme(
                                "theme.spec.mermaid.variables",
                                format!("non-finite number for `{key}` is not supported"),
                            )
                        })?,
                    Value::Bool(value) => MermaidThemeValue::Boolean(*value),
                    _ => {
                        return Err(invalid_theme(
                            "theme.spec.mermaid.variables",
                            format!("`{key}` must be a string, finite number, or boolean"),
                        ));
                    }
                };
            compatibility = compatibility
                .with_variable(key, value)
                .map_err(|error| theme_value_error("theme.spec.mermaid.variables", error))?;
        }
        Ok(compatibility)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingTypographySpecJson {
    default: Option<BindingTextStyleJson>,
    #[serde(default)]
    families: BTreeMap<String, BindingTextStyleJson>,
}

impl BindingTypographySpecJson {
    fn to_typography(&self) -> Result<TypographySpec, BindingError> {
        let mut typography = TypographySpec::default();
        if let Some(style) = &self.default {
            typography = typography.with_default(style.to_text_style()?);
        }
        for (family, style) in &self.families {
            typography = typography.with_family_style(
                parse_render_family(family, "theme.spec.typography.families")?,
                style.to_text_style()?,
            );
        }
        Ok(typography)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingTextStyleJson {
    font_stack: Option<Vec<String>>,
    font_size_px: Option<f32>,
    font_weight: Option<u16>,
    font_style: Option<String>,
    line_height: Option<BindingLineHeightJson>,
    letter_spacing_px: Option<f32>,
    word_spacing_px: Option<f32>,
    transform: Option<String>,
    decoration: Option<String>,
    text_align: Option<String>,
    white_space: Option<String>,
    wrap: Option<String>,
}

impl BindingTextStyleJson {
    fn to_text_style(&self) -> Result<ThemeTextStyle, BindingError> {
        let mut style = ThemeTextStyle::default();
        if let Some(families) = &self.font_stack {
            style =
                style.with_font_stack(FontStack::new(families.iter().cloned()).map_err(
                    |error| theme_value_error("theme.spec.typography.font_stack", error),
                )?);
        }
        if let Some(value) = self.font_size_px {
            style = style
                .with_font_size_px(value)
                .map_err(|error| theme_value_error("theme.spec.typography.font_size_px", error))?;
        }
        if let Some(value) = self.font_weight {
            style = style
                .with_font_weight(value)
                .map_err(|error| theme_value_error("theme.spec.typography.font_weight", error))?;
        }
        if let Some(value) = self.font_style.as_deref() {
            style =
                style.with_font_style(parse_font_style(value, "theme.spec.typography.font_style")?);
        }
        if let Some(value) = &self.line_height {
            style = style
                .with_line_height(value.to_line_height()?)
                .map_err(|error| theme_value_error("theme.spec.typography.line_height", error))?;
        }
        if let Some(value) = self.letter_spacing_px {
            style = style.with_letter_spacing_px(value).map_err(|error| {
                theme_value_error("theme.spec.typography.letter_spacing_px", error)
            })?;
        }
        if let Some(value) = self.word_spacing_px {
            style = style.with_word_spacing_px(value).map_err(|error| {
                theme_value_error("theme.spec.typography.word_spacing_px", error)
            })?;
        }
        if let Some(value) = self.transform.as_deref() {
            style = style.with_transform(parse_text_transform(
                value,
                "theme.spec.typography.transform",
            )?);
        }
        if let Some(value) = self.decoration.as_deref() {
            style = style.with_decoration(parse_text_decoration(
                value,
                "theme.spec.typography.decoration",
            )?);
        }
        if let Some(value) = self.text_align.as_deref() {
            style =
                style.with_text_align(parse_text_align(value, "theme.spec.typography.text_align")?);
        }
        if let Some(value) = self.white_space.as_deref() {
            style = style.with_white_space(parse_white_space(
                value,
                "theme.spec.typography.white_space",
            )?);
        }
        if let Some(value) = self.wrap.as_deref() {
            style = style.with_wrap(parse_wrap_mode(value, "theme.spec.typography.wrap")?);
        }
        Ok(style)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BindingLineHeightJson {
    Keyword(String),
    Multiplier(f32),
    Px(BindingLineHeightPxJson),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingLineHeightPxJson {
    px: f32,
}

impl BindingLineHeightJson {
    fn to_line_height(&self) -> Result<LineHeight, BindingError> {
        match self {
            Self::Keyword(value) if value == "normal" => Ok(LineHeight::Normal),
            Self::Keyword(value) => Err(invalid_theme(
                "theme.spec.typography.line_height",
                format!("unsupported line height `{value}`"),
            )),
            Self::Multiplier(value) => Ok(LineHeight::Multiplier(*value)),
            Self::Px(value) => Ok(LineHeight::Px(value.px)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum BindingStyleEntryJson {
    Rule {
        target: String,
        family: Option<String>,
        variant: Option<String>,
        ordinal: Option<BindingOrdinalSelectorJson>,
        style: BindingThemeStylePatchJson,
    },
    OrdinalPalette {
        target: String,
        colors: Vec<String>,
    },
}

fn binding_styles(entries: &[BindingStyleEntryJson]) -> Result<ThemeRuleSet, BindingError> {
    let mut styles = ThemeRuleSet::default();
    for entry in entries {
        match entry {
            BindingStyleEntryJson::Rule {
                target,
                family,
                variant,
                ordinal,
                style,
            } => {
                let mut rule = ThemeRule::new(
                    parse_theme_target(target, "theme.spec.styles.target")?,
                    style.to_style_patch()?,
                );
                if let Some(family) = family {
                    rule =
                        rule.for_family(parse_render_family(family, "theme.spec.styles.family")?);
                }
                if let Some(variant) = variant {
                    rule = rule
                        .with_variant(parse_theme_variant(variant, "theme.spec.styles.variant")?);
                }
                if let Some(ordinal) = ordinal {
                    rule = rule.with_ordinal(ordinal.to_selector()?);
                }
                styles = styles.with_rule(rule);
            }
            BindingStyleEntryJson::OrdinalPalette { target, colors } => {
                let colors = colors
                    .iter()
                    .map(|color| parse_color(color, "theme.spec.styles.colors"))
                    .collect::<Result<Vec<_>, _>>()?;
                let palette = OrdinalPalette::new(colors).map_err(|error| {
                    theme_value_error("theme.spec.styles.ordinal_palette", error)
                })?;
                styles = styles.with_ordinal_palette(
                    parse_theme_target(target, "theme.spec.styles.target")?,
                    palette,
                );
            }
        }
    }
    Ok(styles)
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BindingOrdinalSelectorJson {
    Exact(BindingOrdinalExactJson),
    Cycle(BindingOrdinalCycleEnvelopeJson),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingOrdinalExactJson {
    exact: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingOrdinalCycleEnvelopeJson {
    cycle: BindingOrdinalCycleJson,
}

impl BindingOrdinalSelectorJson {
    fn to_selector(&self) -> Result<OrdinalSelector, BindingError> {
        match self {
            Self::Exact(value) => OrdinalSelector::exact(value.exact)
                .map_err(|error| theme_value_error("theme.spec.styles.ordinal.exact", error)),
            Self::Cycle(value) => OrdinalSelector::cycle(value.cycle.period, value.cycle.offset)
                .map_err(|error| theme_value_error("theme.spec.styles.ordinal.cycle", error)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingOrdinalCycleJson {
    period: usize,
    offset: usize,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingThemeStylePatchJson {
    #[serde(default, deserialize_with = "deserialize_patch")]
    fill: BindingPatch<BindingCanvasPaintJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    opacity: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    fill_opacity: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    stroke: BindingPatch<BindingStrokePatchJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    radius: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    padding: BindingPatch<BindingInsetsJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    typography: BindingPatch<BindingTextStylePatchJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    effect: BindingPatch<String>,
}

impl BindingThemeStylePatchJson {
    fn to_style_patch(&self) -> Result<ThemeStylePatch, BindingError> {
        let mut patch = ThemeStylePatch::default();
        patch.paint.fill = self.fill.to_specified(|paint| paint.to_paint())?;
        patch.paint.opacity = self.opacity.to_specified_value();
        patch.paint.fill_opacity = self.fill_opacity.to_specified_value();
        match &self.stroke {
            BindingPatch::Omitted => {}
            BindingPatch::Clear => clear_stroke_patch(&mut patch),
            BindingPatch::Value(stroke) => stroke.apply_to(&mut patch)?,
        }
        patch.geometry.radius = self.radius.to_specified_value();
        patch.spacing.padding = self.padding.to_specified(BindingInsetsJson::to_insets)?;
        match &self.typography {
            BindingPatch::Omitted => {}
            BindingPatch::Clear => patch.typography = cleared_text_style_patch(),
            BindingPatch::Value(typography) => {
                patch.typography = typography.to_text_style_patch()?;
            }
        }
        patch.effects.effect = self.effect.to_specified_value();
        Ok(patch)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingStrokePatchJson {
    #[serde(default, deserialize_with = "deserialize_patch")]
    paint: BindingPatch<BindingCanvasPaintJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    width: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    dasharray: BindingPatch<Vec<f32>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    linecap: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    linejoin: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    opacity: BindingPatch<f32>,
}

impl BindingStrokePatchJson {
    fn apply_to(&self, patch: &mut ThemeStylePatch) -> Result<(), BindingError> {
        patch.stroke.paint = self.paint.to_specified(|paint| paint.to_paint())?;
        patch.stroke.width = self.width.to_specified_value();
        patch.stroke.dasharray = self.dasharray.to_specified_value();
        patch.stroke.linecap = self.linecap.to_specified(|value| {
            parse_stroke_linecap(value, "theme.spec.styles.style.stroke.linecap")
        })?;
        patch.stroke.linejoin = self.linejoin.to_specified(|value| {
            parse_stroke_linejoin(value, "theme.spec.styles.style.stroke.linejoin")
        })?;
        patch.stroke.stroke_opacity = self.opacity.to_specified_value();
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingTextStylePatchJson {
    #[serde(default, deserialize_with = "deserialize_patch")]
    font_stack: BindingPatch<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    font_size_px: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    font_weight: BindingPatch<u16>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    font_style: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    line_height: BindingPatch<BindingLineHeightJson>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    letter_spacing_px: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    word_spacing_px: BindingPatch<f32>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    transform: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    decoration: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    text_align: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    white_space: BindingPatch<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    wrap: BindingPatch<String>,
}

impl BindingTextStylePatchJson {
    fn to_text_style_patch(&self) -> Result<TextStylePatch, BindingError> {
        Ok(TextStylePatch {
            font_stack: self.font_stack.to_specified(|families| {
                FontStack::new(families.iter().cloned()).map_err(|error| {
                    theme_value_error("theme.spec.styles.style.typography.font_stack", error)
                })
            })?,
            font_size_px: self.font_size_px.to_specified_value(),
            font_weight: self.font_weight.to_specified_value(),
            font_style: self.font_style.to_specified(|value| {
                parse_font_style(value, "theme.spec.styles.style.typography.font_style")
            })?,
            line_height: self
                .line_height
                .to_specified(BindingLineHeightJson::to_line_height)?,
            letter_spacing_px: self.letter_spacing_px.to_specified_value(),
            word_spacing_px: self.word_spacing_px.to_specified_value(),
            transform: self.transform.to_specified(|value| {
                parse_text_transform(value, "theme.spec.styles.style.typography.transform")
            })?,
            decoration: self.decoration.to_specified(|value| {
                parse_text_decoration(value, "theme.spec.styles.style.typography.decoration")
            })?,
            text_align: self.text_align.to_specified(|value| {
                parse_text_align(value, "theme.spec.styles.style.typography.text_align")
            })?,
            white_space: self.white_space.to_specified(|value| {
                parse_white_space(value, "theme.spec.styles.style.typography.white_space")
            })?,
            wrap: self.wrap.to_specified(|value| {
                parse_wrap_mode(value, "theme.spec.styles.style.typography.wrap")
            })?,
        })
    }
}

#[derive(Debug)]
enum BindingPatch<T> {
    Omitted,
    Clear,
    Value(T),
}

impl<T> Default for BindingPatch<T> {
    fn default() -> Self {
        Self::Omitted
    }
}

impl<T: Clone> BindingPatch<T> {
    fn to_specified_value(&self) -> Specified<T> {
        match self {
            Self::Omitted => Specified::Unspecified,
            Self::Clear => Specified::Clear,
            Self::Value(value) => Specified::Value(value.clone()),
        }
    }
}

impl<T> BindingPatch<T> {
    fn to_specified<U>(
        &self,
        convert: impl FnOnce(&T) -> Result<U, BindingError>,
    ) -> Result<Specified<U>, BindingError> {
        match self {
            Self::Omitted => Ok(Specified::Unspecified),
            Self::Clear => Ok(Specified::Clear),
            Self::Value(value) => convert(value).map(Specified::Value),
        }
    }
}

fn deserialize_patch<'de, D, T>(deserializer: D) -> Result<BindingPatch<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|value| match value {
        Some(value) => BindingPatch::Value(value),
        None => BindingPatch::Clear,
    })
}

fn clear_stroke_patch(patch: &mut ThemeStylePatch) {
    patch.stroke.paint = Specified::Clear;
    patch.stroke.width = Specified::Clear;
    patch.stroke.dasharray = Specified::Clear;
    patch.stroke.linecap = Specified::Clear;
    patch.stroke.linejoin = Specified::Clear;
    patch.stroke.stroke_opacity = Specified::Clear;
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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BindingCanvasPaintJson {
    Color(String),
    Structured(BindingCanvasPaintObjectJson),
}

impl BindingCanvasPaintJson {
    fn to_paint(&self) -> Result<CanvasPaint, BindingError> {
        match self {
            Self::Color(value) if value.trim() == "transparent" => Ok(CanvasPaint::Transparent),
            Self::Color(value) => CanvasPaint::solid(value)
                .map_err(|error| theme_value_error("theme.spec.paint", error)),
            Self::Structured(value) => value.to_paint(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum BindingCanvasPaintObjectJson {
    Transparent,
    Solid {
        color: String,
    },
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<BindingGradientStopJson>,
    },
    RadialGradient {
        center_x: BindingThemeLengthJson,
        center_y: BindingThemeLengthJson,
        radius: BindingThemeLengthJson,
        stops: Vec<BindingGradientStopJson>,
    },
    Pattern {
        pattern: String,
        cell_width: f32,
        cell_height: f32,
        foreground: String,
        background: Option<String>,
        angle_degrees: Option<f32>,
    },
}

impl BindingCanvasPaintObjectJson {
    fn to_paint(&self) -> Result<CanvasPaint, BindingError> {
        match self {
            Self::Transparent => Ok(CanvasPaint::Transparent),
            Self::Solid { color } => CanvasPaint::solid(color)
                .map_err(|error| theme_value_error("theme.spec.paint.color", error)),
            Self::LinearGradient {
                angle_degrees,
                stops,
            } => Ok(CanvasPaint::LinearGradient(
                LinearGradient::new(*angle_degrees, binding_gradient_stops(stops)?).map_err(
                    |error| theme_value_error("theme.spec.paint.linear_gradient", error),
                )?,
            )),
            Self::RadialGradient {
                center_x,
                center_y,
                radius,
                stops,
            } => Ok(CanvasPaint::RadialGradient(
                RadialGradient::new(
                    center_x.to_length(),
                    center_y.to_length(),
                    radius.to_length(),
                    binding_gradient_stops(stops)?,
                )
                .map_err(|error| theme_value_error("theme.spec.paint.radial_gradient", error))?,
            )),
            Self::Pattern {
                pattern,
                cell_width,
                cell_height,
                foreground,
                background,
                angle_degrees,
            } => {
                let mut pattern = PatternSpec::new(
                    parse_pattern_kind(pattern, "theme.spec.paint.pattern")?,
                    *cell_width,
                    *cell_height,
                    parse_color(foreground, "theme.spec.paint.foreground")?,
                )
                .map_err(|error| theme_value_error("theme.spec.paint.pattern", error))?;
                if let Some(background) = background {
                    pattern = pattern
                        .with_background(parse_color(background, "theme.spec.paint.background")?);
                }
                if let Some(angle_degrees) = angle_degrees {
                    pattern = pattern
                        .with_angle_degrees(*angle_degrees)
                        .map_err(|error| {
                            theme_value_error("theme.spec.paint.angle_degrees", error)
                        })?;
                }
                Ok(CanvasPaint::Pattern(pattern))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingGradientStopJson {
    offset: f32,
    color: String,
}

fn binding_gradient_stops(
    stops: &[BindingGradientStopJson],
) -> Result<Vec<GradientStop>, BindingError> {
    stops
        .iter()
        .map(|stop| {
            GradientStop::new(
                stop.offset,
                parse_color(&stop.color, "theme.spec.paint.stops.color")?,
            )
            .map_err(|error| theme_value_error("theme.spec.paint.stops", error))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BindingThemeLengthJson {
    PxValue(f32),
    Px(BindingThemeLengthPxJson),
    Percent(BindingThemeLengthPercentJson),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingThemeLengthPxJson {
    px: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingThemeLengthPercentJson {
    percent: f32,
}

impl BindingThemeLengthJson {
    const fn to_length(&self) -> ThemeLength {
        match self {
            Self::PxValue(value) => ThemeLength::px(*value),
            Self::Px(value) => ThemeLength::px(value.px),
            Self::Percent(value) => ThemeLength::percent(value.percent),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BindingInsetsJson {
    All(f32),
    Sides(BindingInsetsSidesJson),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingInsetsSidesJson {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl BindingInsetsJson {
    const fn to_insets(&self) -> Result<InsetsPx, BindingError> {
        Ok(match self {
            Self::All(value) => InsetsPx::all(*value),
            Self::Sides(value) => InsetsPx {
                top: value.top,
                right: value.right,
                bottom: value.bottom,
                left: value.left,
            },
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingCanvasSpecJson {
    base: Option<BindingCanvasPaintJson>,
    #[serde(default)]
    layers: Vec<BindingCanvasLayerJson>,
    bleed: Option<BindingInsetsJson>,
}

impl BindingCanvasSpecJson {
    fn to_canvas(&self) -> Result<CanvasSpec, BindingError> {
        let mut canvas = CanvasSpec::default();
        if let Some(base) = &self.base {
            canvas = canvas.with_base(base.to_paint()?);
        }
        for layer in &self.layers {
            canvas = canvas
                .with_layer(layer.to_layer()?)
                .map_err(|error| theme_value_error("theme.spec.canvas.layers", error))?;
        }
        if let Some(bleed) = &self.bleed {
            canvas = canvas
                .with_bleed(bleed.to_insets()?)
                .map_err(|error| theme_value_error("theme.spec.canvas.bleed", error))?;
        }
        Ok(canvas)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingCanvasLayerJson {
    paint: BindingCanvasPaintJson,
    opacity: Option<f32>,
    blend_mode: Option<String>,
    offset_x: Option<f32>,
    offset_y: Option<f32>,
}

impl BindingCanvasLayerJson {
    fn to_layer(&self) -> Result<CanvasLayer, BindingError> {
        let mut layer = CanvasLayer::new(self.paint.to_paint()?);
        if let Some(opacity) = self.opacity {
            layer = layer
                .with_opacity(opacity)
                .map_err(|error| theme_value_error("theme.spec.canvas.layers.opacity", error))?;
        }
        if let Some(blend_mode) = self.blend_mode.as_deref() {
            layer = layer.with_blend_mode(parse_blend_mode(
                blend_mode,
                "theme.spec.canvas.layers.blend_mode",
            )?);
        }
        if self.offset_x.is_some() || self.offset_y.is_some() {
            layer = layer
                .with_offset(self.offset_x.unwrap_or(0.0), self.offset_y.unwrap_or(0.0))
                .map_err(|error| theme_value_error("theme.spec.canvas.layers.offset", error))?;
        }
        Ok(layer)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum BindingEffectEntryJson {
    Graph {
        id: String,
        region: BindingFilterRegionJson,
        primitives: Vec<BindingEffectPrimitiveJson>,
    },
    Binding {
        target: String,
        effect_id: String,
    },
}

fn binding_effects(entries: &[BindingEffectEntryJson]) -> Result<DiagramEffectSet, BindingError> {
    let mut effects = DiagramEffectSet::default();
    for entry in entries {
        match entry {
            BindingEffectEntryJson::Graph {
                id,
                region,
                primitives,
            } => {
                let primitives = primitives
                    .iter()
                    .map(BindingEffectPrimitiveJson::to_primitive)
                    .collect::<Result<Vec<_>, _>>()?;
                let graph = EffectGraph::new(id, region.to_region(), primitives)
                    .map_err(|error| theme_value_error("theme.spec.effects.graph", error))?;
                effects = effects
                    .with_graph(graph)
                    .map_err(|error| theme_value_error("theme.spec.effects.graph", error))?;
            }
            BindingEffectEntryJson::Binding { target, effect_id } => {
                effects = effects
                    .with_binding(
                        EffectBinding::new(
                            parse_theme_target(target, "theme.spec.effects.target")?,
                            effect_id,
                        )
                        .map_err(|error| theme_value_error("theme.spec.effects.binding", error))?,
                    )
                    .map_err(|error| theme_value_error("theme.spec.effects.binding", error))?;
            }
        }
    }
    Ok(effects)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFilterRegionJson {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl BindingFilterRegionJson {
    const fn to_region(&self) -> FilterRegion {
        FilterRegion::bounded(self.x, self.y, self.width, self.height)
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum BindingEffectPrimitiveJson {
    DropShadow {
        input: Option<String>,
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread: f32,
        color: String,
    },
    GaussianBlur {
        input: Option<String>,
        std_deviation: f32,
    },
    ColorMatrix {
        input: Option<String>,
        values: Vec<f32>,
    },
    Turbulence {
        input: Option<String>,
        base_frequency_x: f32,
        base_frequency_y: f32,
        octaves: u8,
        seed: i32,
    },
    Displacement {
        input: Option<String>,
        map_input: String,
        scale: f32,
    },
}

impl BindingEffectPrimitiveJson {
    fn to_primitive(&self) -> Result<EffectPrimitive, BindingError> {
        match self {
            Self::DropShadow {
                input,
                offset_x,
                offset_y,
                blur_radius,
                spread,
                color,
            } => Ok(EffectPrimitive::DropShadow {
                input: parse_effect_input(input.as_deref().unwrap_or("source-graphic"))?,
                offset_x: *offset_x,
                offset_y: *offset_y,
                blur_radius: *blur_radius,
                spread: *spread,
                color: parse_color(color, "theme.spec.effects.drop_shadow.color")?,
            }),
            Self::GaussianBlur {
                input,
                std_deviation,
            } => Ok(EffectPrimitive::GaussianBlur {
                input: parse_effect_input(input.as_deref().unwrap_or("source-graphic"))?,
                std_deviation: *std_deviation,
            }),
            Self::ColorMatrix { input, values } => {
                let values: [f32; 20] = values.clone().try_into().map_err(|_| {
                    invalid_theme(
                        "theme.spec.effects.color_matrix.values",
                        "color matrix must contain exactly 20 numbers",
                    )
                })?;
                Ok(EffectPrimitive::ColorMatrix {
                    input: parse_effect_input(input.as_deref().unwrap_or("source-graphic"))?,
                    values,
                })
            }
            Self::Turbulence {
                input,
                base_frequency_x,
                base_frequency_y,
                octaves,
                seed,
            } => Ok(EffectPrimitive::Turbulence {
                input: parse_effect_input(input.as_deref().unwrap_or("source-graphic"))?,
                base_frequency_x: *base_frequency_x,
                base_frequency_y: *base_frequency_y,
                octaves: *octaves,
                seed: *seed,
            }),
            Self::Displacement {
                input,
                map_input,
                scale,
            } => Ok(EffectPrimitive::Displacement {
                input: parse_effect_input(input.as_deref().unwrap_or("source-graphic"))?,
                map_input: parse_effect_input(map_input)?,
                scale: *scale,
            }),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingThemeRequirementsJson {
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    text_capabilities: Vec<String>,
}

impl BindingThemeRequirementsJson {
    fn to_requirements(&self) -> Result<ThemeRequirements, BindingError> {
        let capabilities = self
            .capabilities
            .iter()
            .map(|id| parse_theme_capability(id))
            .collect::<Result<Vec<_>, _>>()?;
        let text_capabilities = self
            .text_capabilities
            .iter()
            .map(|id| parse_text_layout_capability(id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ThemeRequirements::new()
            .with_required_capabilities(capabilities)
            .with_required_text_capabilities(text_capabilities))
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingThemeAssetsJson {
    #[serde(default)]
    fonts: Vec<BindingFontAssetJson>,
    #[serde(default)]
    aliases: Vec<BindingFontAliasJson>,
    #[serde(default)]
    generic_families: Vec<BindingGenericFamilyJson>,
    available_sources: Option<Vec<String>>,
    embedding: Option<String>,
}

impl BindingThemeAssetsJson {
    fn to_font_catalog(
        &self,
        resources: &ThemeResourcePolicy,
    ) -> Result<Option<FontCatalogSpec>, BindingError> {
        if self.fonts.is_empty() {
            if self.aliases.is_empty()
                && self.generic_families.is_empty()
                && self.available_sources.is_none()
                && self.embedding.is_none()
            {
                return Ok(None);
            }
            return Err(invalid_theme(
                "theme.spec.assets.fonts",
                "font catalog settings require at least one embedded font",
            ));
        }

        let encoded_bytes = self.fonts.iter().try_fold(0usize, |total, asset| {
            total.checked_add(asset.data_base64.len()).ok_or_else(|| {
                invalid_theme(
                    "theme.spec.assets.fonts.data_base64",
                    "aggregate base64 length overflowed",
                )
            })
        })?;
        resources
            .check_theme_base64_bytes(encoded_bytes)
            .map_err(theme_resource_error)?;

        let mut font_specs = Vec::with_capacity(self.fonts.len());
        for font in &self.fonts {
            let bytes = decode_canonical_base64(&font.data_base64)?;
            font_specs.push(FontAssetSpec::new(&font.id, bytes));
        }
        let mut catalog = FontCatalogSpec::new(font_specs);
        for alias in &self.aliases {
            catalog = catalog.with_alias(&alias.alias, &alias.target);
        }
        for mapping in &self.generic_families {
            catalog = catalog.with_generic_family(
                parse_generic_font_family(&mapping.generic)?,
                &mapping.target,
            );
        }
        if let Some(sources) = &self.available_sources {
            catalog = catalog.with_available_sources(
                sources
                    .iter()
                    .map(|source| parse_font_source(source))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        if let Some(embedding) = self.embedding.as_deref() {
            catalog = catalog.with_embedding_requirement(parse_embedding_requirement(embedding)?);
        }
        Ok(Some(catalog))
    }

    fn expected_font_formats(&self) -> Result<BTreeMap<String, FontContainer>, BindingError> {
        let mut expected = BTreeMap::new();
        for font in &self.fonts {
            let format = parse_font_container(&font.format)?;
            if expected.insert(font.id.clone(), format).is_some() {
                return Err(invalid_theme(
                    "theme.spec.assets.fonts.id",
                    format!("duplicate font asset id `{}`", font.id),
                ));
            }
        }
        Ok(expected)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFontAssetJson {
    id: String,
    format: String,
    data_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingFontAliasJson {
    alias: String,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingGenericFamilyJson {
    generic: String,
    target: String,
}

fn decode_canonical_base64(value: &str) -> Result<Vec<u8>, BindingError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| {
            invalid_theme(
                "theme.spec.assets.fonts.data_base64",
                format!("invalid standard base64: {error}"),
            )
        })?;
    if base64::engine::general_purpose::STANDARD.encode(&bytes) != value {
        return Err(invalid_theme(
            "theme.spec.assets.fonts.data_base64",
            "font data must use canonical padded standard base64 without whitespace",
        ));
    }
    Ok(bytes)
}

fn validate_compiled_font_formats(
    theme: &DiagramTheme,
    expected: &BTreeMap<String, FontContainer>,
) -> Result<(), BindingError> {
    for asset in theme.font_catalog().assets() {
        let Some(expected) = expected.get(asset.id()) else {
            continue;
        };
        if asset.input_container() != *expected {
            return Err(invalid_theme(
                "theme.spec.assets.fonts.format",
                format!(
                    "font asset `{}` declares `{}` but contains `{}` data",
                    asset.id(),
                    expected.id(),
                    asset.input_container().id()
                ),
            ));
        }
    }
    Ok(())
}

fn parse_color(value: &str, field: &'static str) -> Result<ThemeColorValue, BindingError> {
    ThemeColorValue::parse(value).map_err(|error| theme_value_error(field, error))
}

fn parse_render_family(value: &str, field: &'static str) -> Result<RenderFamilyKind, BindingError> {
    RenderFamilyKind::from_id(value).ok_or_else(|| unsupported(field, value))
}

fn parse_theme_target(value: &str, field: &'static str) -> Result<ThemeTarget, BindingError> {
    let target = match value {
        "canvas" => ThemeTarget::Canvas,
        "node" => ThemeTarget::Node,
        "node-label" => ThemeTarget::NodeLabel,
        "edge" => ThemeTarget::Edge,
        "edge-label" => ThemeTarget::EdgeLabel,
        "edge-label-background" => ThemeTarget::EdgeLabelBackground,
        "cluster" => ThemeTarget::Cluster,
        "cluster-label" => ThemeTarget::ClusterLabel,
        "marker" => ThemeTarget::Marker,
        "title" => ThemeTarget::Title,
        "text" => ThemeTarget::Text,
        "axis" => ThemeTarget::Axis,
        "legend" => ThemeTarget::Legend,
        "table" => ThemeTarget::Table,
        "task" => ThemeTarget::Task,
        "state" => ThemeTarget::State,
        "state-label" => ThemeTarget::StateLabel,
        "transition" => ThemeTarget::Transition,
        "transition-marker" => ThemeTarget::TransitionMarker,
        "transition-label" => ThemeTarget::TransitionLabel,
        "transition-label-background" => ThemeTarget::TransitionLabelBackground,
        "composite" => ThemeTarget::Composite,
        "composite-header" => ThemeTarget::CompositeHeader,
        "composite-label" => ThemeTarget::CompositeLabel,
        "special-state" => ThemeTarget::SpecialState,
        "special-state-inner" => ThemeTarget::SpecialStateInner,
        "actor" => ThemeTarget::Actor,
        "actor-label" => ThemeTarget::ActorLabel,
        "lifeline" => ThemeTarget::Lifeline,
        "message" => ThemeTarget::Message,
        "message-label" => ThemeTarget::MessageLabel,
        "loop" => ThemeTarget::Loop,
        "loop-label" => ThemeTarget::LoopLabel,
        "note" => ThemeTarget::Note,
        "note-label" => ThemeTarget::NoteLabel,
        "activation" => ThemeTarget::Activation,
        "requirement" => ThemeTarget::Requirement,
        "relation" => ThemeTarget::Relation,
        "pie-slice" => ThemeTarget::PieSlice,
        "chart-series" => ThemeTarget::ChartSeries,
        "timeline-event" => ThemeTarget::TimelineEvent,
        "journey-task" => ThemeTarget::JourneyTask,
        _ => return Err(unsupported(field, value)),
    };
    Ok(target)
}

fn parse_theme_variant(value: &str, field: &'static str) -> Result<ThemeVariant, BindingError> {
    let variant = match value {
        "default" => ThemeVariant::Default,
        "primary" => ThemeVariant::Primary,
        "secondary" => ThemeVariant::Secondary,
        "tertiary" => ThemeVariant::Tertiary,
        "active" => ThemeVariant::Active,
        "selected" => ThemeVariant::Selected,
        "odd" => ThemeVariant::Odd,
        "even" => ThemeVariant::Even,
        "start" => ThemeVariant::Start,
        "end" => ThemeVariant::End,
        "special" => ThemeVariant::Special,
        "error" => ThemeVariant::Error,
        "warning" => ThemeVariant::Warning,
        "success" => ThemeVariant::Success,
        _ => return Err(unsupported(field, value)),
    };
    Ok(variant)
}

fn parse_font_style(value: &str, field: &'static str) -> Result<FontStyle, BindingError> {
    match value {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_text_transform(value: &str, field: &'static str) -> Result<TextTransform, BindingError> {
    match value {
        "none" => Ok(TextTransform::None),
        "uppercase" => Ok(TextTransform::Uppercase),
        "lowercase" => Ok(TextTransform::Lowercase),
        "capitalize" => Ok(TextTransform::Capitalize),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_text_decoration(value: &str, field: &'static str) -> Result<TextDecoration, BindingError> {
    match value {
        "none" => Ok(TextDecoration::None),
        "underline" => Ok(TextDecoration::Underline),
        "overline" => Ok(TextDecoration::Overline),
        "line-through" => Ok(TextDecoration::LineThrough),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_text_align(value: &str, field: &'static str) -> Result<TextAlign, BindingError> {
    match value {
        "start" => Ok(TextAlign::Start),
        "center" => Ok(TextAlign::Center),
        "end" => Ok(TextAlign::End),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_white_space(value: &str, field: &'static str) -> Result<WhiteSpace, BindingError> {
    match value {
        "normal" => Ok(WhiteSpace::Normal),
        "pre" => Ok(WhiteSpace::Pre),
        "no-wrap" => Ok(WhiteSpace::NoWrap),
        "pre-wrap" => Ok(WhiteSpace::PreWrap),
        "pre-line" => Ok(WhiteSpace::PreLine),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_wrap_mode(value: &str, field: &'static str) -> Result<ThemeWrapMode, BindingError> {
    match value {
        "normal" => Ok(ThemeWrapMode::Normal),
        "break-word" => Ok(ThemeWrapMode::BreakWord),
        "anywhere" => Ok(ThemeWrapMode::Anywhere),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_stroke_linecap(value: &str, field: &'static str) -> Result<StrokeLineCap, BindingError> {
    match value {
        "butt" => Ok(StrokeLineCap::Butt),
        "round" => Ok(StrokeLineCap::Round),
        "square" => Ok(StrokeLineCap::Square),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_stroke_linejoin(value: &str, field: &'static str) -> Result<StrokeLineJoin, BindingError> {
    match value {
        "miter" => Ok(StrokeLineJoin::Miter),
        "round" => Ok(StrokeLineJoin::Round),
        "bevel" => Ok(StrokeLineJoin::Bevel),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_pattern_kind(value: &str, field: &'static str) -> Result<PatternKind, BindingError> {
    match value {
        "dots" => Ok(PatternKind::Dots),
        "grid" => Ok(PatternKind::Grid),
        "stripes" => Ok(PatternKind::Stripes),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_blend_mode(value: &str, field: &'static str) -> Result<BlendMode, BindingError> {
    match value {
        "normal" => Ok(BlendMode::Normal),
        "multiply" => Ok(BlendMode::Multiply),
        "screen" => Ok(BlendMode::Screen),
        "overlay" => Ok(BlendMode::Overlay),
        "darken" => Ok(BlendMode::Darken),
        "lighten" => Ok(BlendMode::Lighten),
        "difference" => Ok(BlendMode::Difference),
        _ => Err(unsupported(field, value)),
    }
}

fn parse_effect_input(value: &str) -> Result<EffectInput, BindingError> {
    match value {
        "source-graphic" => Ok(EffectInput::SourceGraphic),
        "previous" => Ok(EffectInput::Previous),
        _ => Err(unsupported("theme.spec.effects.input", value)),
    }
}

fn parse_theme_capability(value: &str) -> Result<ThemeCapability, BindingError> {
    ThemeCapability::ALL
        .iter()
        .copied()
        .find(|capability| capability.id() == value)
        .ok_or_else(|| unsupported("theme.spec.requirements.capabilities", value))
}

fn parse_text_layout_capability(value: &str) -> Result<TextLayoutCapability, BindingError> {
    TextLayoutCapability::ALL
        .iter()
        .copied()
        .find(|capability| capability.id() == value)
        .ok_or_else(|| unsupported("theme.spec.requirements.text_capabilities", value))
}

fn parse_font_source(value: &str) -> Result<FontSource, BindingError> {
    match value {
        "embedded" => Ok(FontSource::Embedded),
        "system" => Ok(FontSource::System),
        _ => Err(unsupported("theme.spec.assets.available_sources", value)),
    }
}

fn parse_embedding_requirement(value: &str) -> Result<FontEmbeddingRequirement, BindingError> {
    match value {
        "none" => Ok(FontEmbeddingRequirement::NoEmbedding),
        "full-font" => Ok(FontEmbeddingRequirement::FullFont),
        "subset" => Ok(FontEmbeddingRequirement::Subset),
        _ => Err(unsupported("theme.spec.assets.embedding", value)),
    }
}

fn parse_generic_font_family(value: &str) -> Result<GenericFontFamily, BindingError> {
    match value {
        "serif" => Ok(GenericFontFamily::Serif),
        "sans-serif" => Ok(GenericFontFamily::SansSerif),
        "monospace" => Ok(GenericFontFamily::Monospace),
        "cursive" => Ok(GenericFontFamily::Cursive),
        "fantasy" => Ok(GenericFontFamily::Fantasy),
        "system-ui" => Ok(GenericFontFamily::SystemUi),
        _ => Err(unsupported(
            "theme.spec.assets.generic_families.generic",
            value,
        )),
    }
}

fn parse_font_container(value: &str) -> Result<FontContainer, BindingError> {
    match value {
        "truetype" => Ok(FontContainer::TrueType),
        "opentype" => Ok(FontContainer::OpenType),
        "collection" => Ok(FontContainer::Collection),
        "woff2" => Ok(FontContainer::Woff2),
        _ => Err(unsupported("theme.spec.assets.fonts.format", value)),
    }
}

fn theme_compile_error(error: merman::svg::ThemeCompileError) -> BindingError {
    match error {
        merman::svg::ThemeCompileError::ResourceLimit(error)
        | merman::svg::ThemeCompileError::FontCatalog(
            merman::svg::FontCatalogError::ResourceLimit(error),
        ) => theme_resource_error(error),
        error => invalid_theme("theme", error.to_string()),
    }
}

fn theme_resource_error(error: merman::svg::ThemeResourceLimitExceeded) -> BindingError {
    let actual = u64::try_from(error.actual).unwrap_or(u64::MAX);
    let max = u64::try_from(error.max).unwrap_or(u64::MAX);
    let profile = error
        .profile
        .map(merman::resources::ResourceProfile::id)
        .unwrap_or("custom-theme-policy");
    BindingError::resource_limit(
        error.phase.as_str(),
        error.limit,
        actual,
        max,
        profile,
        error.to_string(),
    )
}

fn theme_value_error(
    field: &'static str,
    error: merman::svg::ThemeCompileValidationError,
) -> BindingError {
    invalid_theme(field, error.to_string())
}

fn unsupported(field: &'static str, value: &str) -> BindingError {
    invalid_theme(field, format!("unsupported value `{value}`"))
}

fn invalid_theme(field: &'static str, message: impl Into<String>) -> BindingError {
    BindingError::new(
        BindingStatus::InvalidArgument,
        format!("invalid {field}: {}", message.into()),
    )
}

fn invalid_options(message: impl Into<String>) -> BindingError {
    BindingError::new(BindingStatus::OptionsJsonError, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_semantic_target_id_round_trips_through_the_binding_parser() {
        for &target in ThemeTarget::ALL {
            let id = target.id();
            assert_eq!(
                parse_theme_target(id, "theme.spec.styles.target").unwrap(),
                target,
                "binding theme parser must accept semantic target `{id}`"
            );
        }
    }

    #[test]
    fn every_render_family_id_round_trips_through_the_binding_parser() {
        for &family in RenderFamilyKind::all() {
            let id = family.as_str();
            assert_eq!(
                parse_render_family(id, "theme.spec.styles.family").unwrap(),
                family,
                "binding theme parser must accept typed render family `{id}`"
            );
        }
    }

    #[test]
    fn binding_mermaid_theme_ids_follow_the_core_catalog() {
        for &theme in MermaidThemeId::ALL {
            let wire: BindingMermaidCompatibilityJson =
                serde_json::from_value(serde_json::json!({ "theme": theme.as_str() })).unwrap();
            let compatibility = wire.to_compatibility().unwrap();
            assert_eq!(compatibility.theme(), Some(theme));
        }
    }

    #[test]
    fn binding_rejects_unknown_mermaid_theme_ids() {
        for invalid in ["unknown", "null"] {
            let wire: BindingMermaidCompatibilityJson =
                serde_json::from_value(serde_json::json!({ "theme": invalid })).unwrap();
            let error = wire.to_compatibility().unwrap_err();

            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(
                error
                    .message()
                    .contains(&format!("unsupported Mermaid theme `{invalid}`"))
            );
        }
    }

    #[test]
    fn theme_wire_requires_exactly_one_selection() {
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"preset": "editor-dark", "spec": {}}),
            serde_json::json!({"preset": null}),
            serde_json::json!({"spec": null}),
        ] {
            let error = validate_theme_wire(Some(&invalid)).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
        validate_theme_wire(Some(&Value::Null)).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"preset": "editor-dark"}))).unwrap();
        validate_theme_wire(Some(&serde_json::json!({"spec": {}}))).unwrap();
    }

    #[test]
    fn preset_and_structured_spec_compile_to_typed_themes() {
        let preset: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
            "preset": "editor-dark"
        }))
        .unwrap();
        let preset = compile_theme_with(&DiagramThemeCompiler::new(), Some(&preset))
            .unwrap()
            .unwrap();
        assert!(
            preset
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );

        let spec: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
            "spec": {
                "typography": {
                    "default": {
                        "font_stack": ["Inter", "sans-serif"],
                        "font_size_px": 18.0,
                        "line_height": 1.4
                    }
                },
                "styles": [
                    {
                        "kind": "rule",
                        "target": "node",
                        "family": "flowchart",
                        "style": {
                            "fill": "#101820",
                            "stroke": { "paint": "#f2aa4c", "width": 2.0 },
                            "radius": 6.0
                        }
                    },
                    {
                        "kind": "ordinal-palette",
                        "target": "chart-series",
                        "colors": ["#f2aa4c", "#4cc9f0"]
                    }
                ],
                "canvas": { "base": "#0b0f14", "bleed": 8.0 },
                "requirements": { "capabilities": ["rounded-geometry"] }
            }
        }))
        .unwrap();
        let theme = compile_theme_with(&DiagramThemeCompiler::new(), Some(&spec))
            .unwrap()
            .unwrap();
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::Typography)
        );
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::RoundedGeometry)
        );
    }

    #[test]
    fn style_patch_null_is_an_explicit_clear() {
        let spec: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
            "spec": {
                "styles": [{
                    "kind": "rule",
                    "target": "node",
                    "style": {
                        "fill": null,
                        "stroke": null,
                        "typography": null
                    }
                }]
            }
        }))
        .unwrap();
        let compiler = DiagramThemeCompiler::new();
        let typed = spec
            .spec
            .as_ref()
            .expect("structured spec")
            .to_theme_spec(compiler.resource_policy())
            .unwrap();
        let rule = &typed.styles().rules()[0];
        assert!(matches!(rule.style().paint.fill, Specified::Clear));
        assert!(matches!(rule.style().stroke.paint, Specified::Clear));
        assert!(matches!(
            rule.style().typography.font_stack,
            Specified::Clear
        ));
        let theme = compile_theme_with(&compiler, Some(&spec)).unwrap().unwrap();
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );
    }

    #[test]
    fn font_data_requires_canonical_standard_base64() {
        let error = decode_canonical_base64("Zg").unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("standard base64"));
        assert_eq!(decode_canonical_base64("Zg==").unwrap(), b"f");
    }

    #[test]
    fn nested_theme_value_objects_reject_unknown_fields() {
        let invalid = [
            serde_json::json!({
                "spec": {
                    "typography": {
                        "default": { "line_height": { "px": 18.0, "extra": true } }
                    }
                }
            }),
            serde_json::json!({
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": "node",
                        "ordinal": { "exact": 1, "extra": true },
                        "style": {}
                    }]
                }
            }),
            serde_json::json!({
                "spec": {
                    "canvas": {
                        "base": {
                            "kind": "radial-gradient",
                            "center_x": { "px": 10.0, "extra": true },
                            "center_y": { "percent": 50.0 },
                            "radius": { "percent": 50.0 },
                            "stops": [
                                { "offset": 0.0, "color": "#000000" },
                                { "offset": 1.0, "color": "#ffffff" }
                            ]
                        }
                    }
                }
            }),
            serde_json::json!({
                "spec": {
                    "canvas": {
                        "bleed": {
                            "top": 1.0,
                            "right": 1.0,
                            "bottom": 1.0,
                            "left": 1.0,
                            "extra": true
                        }
                    }
                }
            }),
        ];

        for value in invalid {
            assert!(
                serde_json::from_value::<BindingThemeOptionsJson>(value).is_err(),
                "nested unknown field must fail closed"
            );
        }
    }

    #[test]
    fn raw_theme_input_limit_precedes_typed_json_allocation() {
        let policy = ThemeResourcePolicy::default();
        let max = policy
            .value(merman::svg::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("interactive encoded-theme ceiling");
        let padding = " ".repeat(max);
        let options = format!(r#"{{"theme":{{{padding}"preset":"editor-light"}}}}"#);

        let error = validate_theme_input_json(options.as_bytes()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error
            .resource_details()
            .expect("structured theme limit details");
        assert_eq!(details.limit_id, "max_theme_encoded_bytes");
        assert_eq!(details.phase, "theme_input");
        assert_eq!(details.max, u64::try_from(max).unwrap());
        assert!(details.actual > details.max);
        assert_eq!(details.profile, "interactive");
    }

    #[test]
    fn standalone_theme_selection_uses_the_binding_schema_and_recipe_compiler() {
        let theme = compile_theme_selection_json(br#"{"preset":"editor-dark"}"#).unwrap();
        assert_ne!(theme.recipe_fingerprint().as_bytes(), &[0; 32]);

        let theme = compile_theme_selection_json_with(
            &DiagramThemeCompiler::new(),
            br##"{
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": "node",
                        "style": { "fill": "#111827" }
                    }]
                }
            }"##,
        )
        .expect("host-independent recipe compilation");
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::SemanticRules)
        );
    }

    #[test]
    fn mermaid_compatibility_preserves_supported_scalar_types() {
        let spec: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
            "spec": {
                "mermaid": {
                    "variables": {
                        "fontSize": 16,
                        "darkMode": true,
                        "primaryColor": "#123456",
                        "useMaxWidth": false
                    }
                }
            }
        }))
        .unwrap();

        let theme = compile_theme_with(&DiagramThemeCompiler::new(), Some(&spec))
            .unwrap()
            .unwrap();
        let compiler = DiagramThemeCompiler::new();
        let typed = spec
            .spec
            .as_ref()
            .expect("structured spec")
            .to_theme_spec(compiler.resource_policy())
            .unwrap();
        let compatibility = typed.mermaid();
        assert_eq!(compatibility.dark_mode(), Some(true));
        let variables = compatibility.variables().collect::<BTreeMap<_, _>>();
        assert_eq!(variables.len(), 3);
        assert_eq!(
            variables.get("fontSize"),
            Some(&&MermaidThemeValue::Number(16.0))
        );
        assert_eq!(
            variables.get("primaryColor"),
            Some(&&MermaidThemeValue::String("#123456".to_string()))
        );
        assert_eq!(
            variables.get("useMaxWidth"),
            Some(&&MermaidThemeValue::Boolean(false))
        );
        assert!(theme.recipe_fingerprint().as_bytes() != &[0; 32]);
    }

    #[test]
    fn mermaid_compatibility_rejects_structured_variable_values() {
        for value in [
            serde_json::json!(["Inter", "sans-serif"]),
            serde_json::json!({
                "value": "#123456"
            }),
            Value::Null,
        ] {
            let spec: BindingThemeOptionsJson = serde_json::from_value(serde_json::json!({
                "spec": {
                    "mermaid": {
                        "variables": { "primaryColor": value }
                    }
                }
            }))
            .unwrap();

            let error = compile_theme_with(&DiagramThemeCompiler::new(), Some(&spec)).unwrap_err();
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(
                error
                    .message()
                    .contains("string, finite number, or boolean")
            );
        }
    }
}
