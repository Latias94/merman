use sha2::{Digest, Sha256};

use super::admission::ThemeRequirements;
use super::canvas::{CanvasPaint, CanvasSpec, InsetsPx, PatternKind, ThemeColorValue, ThemeLength};
use super::effects::{DiagramEffectSet, EffectInput, EffectPrimitive};
use super::semantic::{
    OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeRuleSet, ThemeStylePatch,
};
use super::spec::{DiagramThemeSpec, MermaidThemeCompatibility, MermaidThemeValue};
use super::typography::{
    LineHeight, Specified, TextAlign, TextDecoration, TextStyle, TextStylePatch, TextTransform,
    TypographySpec, WhiteSpace, WrapMode,
};
use super::{FontCatalog, FontStack, FontStyle};

const RECIPE_FINGERPRINT_DOMAIN: &[u8] = b"merman-theme-recipe-v4";

pub(crate) fn recipe_fingerprint(
    spec: &DiagramThemeSpec,
    catalog: &FontCatalog,
    requirements: &ThemeRequirements,
) -> [u8; 32] {
    let mut encoder = CanonicalEncoder::new(RECIPE_FINGERPRINT_DOMAIN);
    encoder.field("mermaid", |encoder| encode_mermaid(encoder, spec.mermaid()));
    encoder.field("typography", |encoder| {
        encode_typography(encoder, spec.typography())
    });
    encoder.field("styles", |encoder| encode_rule_set(encoder, spec.styles()));
    encoder.field("canvas", |encoder| encode_canvas(encoder, spec.canvas()));
    encoder.field("effects", |encoder| encode_effects(encoder, spec.effects()));
    encoder.field("font-catalog", |encoder| {
        encoder.bytes(catalog.fingerprint().as_bytes())
    });
    encoder.field("requirements", |encoder| {
        encoder.field("theme-capabilities", |encoder| {
            let required = requirements.required_capabilities().collect::<Vec<_>>();
            encoder.sequence_len(required.len());
            for capability in required {
                encoder.string(capability.id());
            }
        });
        encoder.field("text-layout-capabilities", |encoder| {
            let text = requirements
                .required_text_capabilities()
                .collect::<Vec<_>>();
            encoder.sequence_len(text.len());
            for capability in text {
                encoder.string(capability.id());
            }
        });
    });
    encoder.finish()
}

fn encode_mermaid(encoder: &mut CanonicalEncoder, mermaid: &MermaidThemeCompatibility) {
    encoder.option_string("theme", mermaid.theme_name());
    encoder.field("dark-mode", |encoder| match mermaid.dark_mode() {
        Some(value) => {
            encoder.byte(1);
            encoder.boolean(value);
        }
        None => encoder.byte(0),
    });
    let variables = mermaid.variables().collect::<Vec<_>>();
    encoder.field("variables", |encoder| {
        encoder.sequence_len(variables.len());
        for (key, value) in variables {
            encoder.field("variable", |encoder| {
                encoder.field("name", |encoder| encoder.string(key));
                encoder.field("value", |encoder| match value {
                    MermaidThemeValue::String(value) => {
                        encoder.field("kind", |encoder| encoder.string("string"));
                        encoder.field("string", |encoder| encoder.string(value));
                    }
                    MermaidThemeValue::Number(value) => {
                        encoder.field("kind", |encoder| encoder.string("number"));
                        encoder.field("number", |encoder| encoder.f64(*value));
                    }
                    MermaidThemeValue::Boolean(value) => {
                        encoder.field("kind", |encoder| encoder.string("boolean"));
                        encoder.field("boolean", |encoder| encoder.boolean(*value));
                    }
                });
            });
        }
    });
}

fn encode_typography(encoder: &mut CanonicalEncoder, typography: &TypographySpec) {
    encoder.field("default", |encoder| {
        encode_text_style(encoder, typography.default_style())
    });
    let overrides = typography.family_overrides().collect::<Vec<_>>();
    encoder.field("family-overrides", |encoder| {
        encoder.sequence_len(overrides.len());
        for (family, style) in overrides {
            encoder.field("family-override", |encoder| {
                encoder.field("family", |encoder| encoder.string(family));
                encoder.field("style", |encoder| encode_text_style(encoder, style));
            });
        }
    });
}

fn encode_text_style(encoder: &mut CanonicalEncoder, style: &TextStyle) {
    encoder.field("specified-properties", |encoder| {
        let properties = style.specified_properties().collect::<Vec<_>>();
        encoder.sequence_len(properties.len());
        for property in properties {
            encoder.string(property.id());
        }
    });
    encoder.field("font-stack", |encoder| {
        encode_font_stack(encoder, style.font_stack())
    });
    encoder.field("font-size-px", |encoder| encoder.f32(style.font_size_px()));
    encoder.field("font-weight", |encoder| {
        encoder.u64(u64::from(style.font_weight()))
    });
    encoder.field("font-style", |encoder| {
        encoder.string(style.font_style().id())
    });
    encoder.field("line-height", |encoder| {
        encode_line_height(encoder, style.line_height())
    });
    encoder.field("letter-spacing-px", |encoder| {
        encoder.f32(style.letter_spacing_px())
    });
    encoder.field("word-spacing-px", |encoder| {
        encoder.f32(style.word_spacing_px())
    });
    encoder.field("transform", |encoder| {
        encoder.string(text_transform_id(style.transform()))
    });
    encoder.field("decoration", |encoder| {
        encoder.string(text_decoration_id(style.decoration()))
    });
    encoder.field("text-align", |encoder| {
        encoder.string(text_align_id(style.text_align()))
    });
    encoder.field("white-space", |encoder| {
        encoder.string(white_space_id(style.white_space()))
    });
    encoder.field("wrap", |encoder| encoder.string(wrap_mode_id(style.wrap())));
}

fn encode_font_stack(encoder: &mut CanonicalEncoder, stack: &FontStack) {
    encoder.field("families", |encoder| {
        encoder.sequence_len(stack.families().len());
        for family in stack.families() {
            encoder.string(family);
        }
    });
}

fn encode_line_height(encoder: &mut CanonicalEncoder, line_height: LineHeight) {
    match line_height {
        LineHeight::Normal => encoder.field("kind", |encoder| encoder.string("normal")),
        LineHeight::Multiplier(value) => {
            encoder.field("kind", |encoder| encoder.string("multiplier"));
            encoder.field("value", |encoder| encoder.f32(value));
        }
        LineHeight::Px(value) => {
            encoder.field("kind", |encoder| encoder.string("px"));
            encoder.field("value", |encoder| encoder.f32(value));
        }
    }
}

fn encode_rule_set(encoder: &mut CanonicalEncoder, rules: &ThemeRuleSet) {
    encoder.field("rules", |encoder| {
        encoder.sequence_len(rules.rules().len());
        for rule in rules.rules() {
            encoder.field("rule", |encoder| {
                encoder.field("target", |encoder| encoder.string(rule.target().id()));
                encoder.option_string("family", rule.family().map(|family| family.as_str()));
                encoder.option_string("variant", rule.variant().map(|variant| variant.id()));
                encoder.field("ordinal", |encoder| match rule.ordinal() {
                    None => encoder.byte(0),
                    Some(OrdinalSelector::Exact(index)) => {
                        encoder.byte(1);
                        encoder.field("index", |encoder| encoder.usize(index));
                    }
                    Some(OrdinalSelector::Cycle { period, offset }) => {
                        encoder.byte(2);
                        encoder.field("period", |encoder| encoder.usize(period));
                        encoder.field("offset", |encoder| encoder.usize(offset));
                    }
                });
                encoder.field("style", |encoder| encode_style_patch(encoder, rule.style()));
            });
        }
    });

    let mut palettes = rules.ordinal_palettes().iter().collect::<Vec<_>>();
    palettes.sort_by_key(|(target, _)| target.id());
    encoder.field("ordinal-palettes", |encoder| {
        encoder.sequence_len(palettes.len());
        for (target, palette) in palettes {
            encoder.field("ordinal-palette", |encoder| {
                encoder.field("target", |encoder| encoder.string(target.id()));
                encoder.field("colors", |encoder| {
                    encoder.sequence_len(palette.colors().len());
                    for color in palette.colors() {
                        encode_color(encoder, color);
                    }
                });
            });
        }
    });
}

fn encode_style_patch(encoder: &mut CanonicalEncoder, style: &ThemeStylePatch) {
    encoder.specified("fill", &style.paint.fill, encode_canvas_paint);
    encoder.specified_f32("opacity", &style.paint.opacity);
    encoder.specified_f32("fill-opacity", &style.paint.fill_opacity);
    encoder.specified("stroke", &style.stroke.paint, encode_canvas_paint);
    encoder.specified_f32("stroke-width", &style.stroke.width);
    encoder.specified(
        "stroke-dasharray",
        &style.stroke.dasharray,
        |encoder, values| {
            encoder.sequence_len(values.len());
            for value in values {
                encoder.f32(*value);
            }
        },
    );
    encoder.specified("stroke-linecap", &style.stroke.linecap, |encoder, value| {
        encoder.string(stroke_linecap_id(*value));
    });
    encoder.specified(
        "stroke-linejoin",
        &style.stroke.linejoin,
        |encoder, value| {
            encoder.string(stroke_linejoin_id(*value));
        },
    );
    encoder.specified_f32("stroke-opacity", &style.stroke.stroke_opacity);
    encoder.specified_f32("radius", &style.geometry.radius);
    encoder.specified("padding", &style.spacing.padding, |encoder, value| {
        encode_insets(encoder, *value)
    });
    encode_text_style_patch(encoder, &style.typography);
    encoder.specified("effect", &style.effects.effect, |encoder, value| {
        encoder.string(value)
    });
}

fn encode_text_style_patch(encoder: &mut CanonicalEncoder, style: &TextStylePatch) {
    encoder.specified("font-stack", &style.font_stack, encode_font_stack);
    encoder.specified_f32("font-size", &style.font_size_px);
    encoder.specified("font-weight", &style.font_weight, |encoder, value| {
        encoder.u64(u64::from(*value))
    });
    encoder.specified("font-style", &style.font_style, |encoder, value| {
        encoder.string(font_style_id(*value))
    });
    encoder.specified("line-height", &style.line_height, |encoder, value| {
        encode_line_height(encoder, *value)
    });
    encoder.specified_f32("letter-spacing", &style.letter_spacing_px);
    encoder.specified_f32("word-spacing", &style.word_spacing_px);
    encoder.specified("transform", &style.transform, |encoder, value| {
        encoder.string(text_transform_id(*value))
    });
    encoder.specified("decoration", &style.decoration, |encoder, value| {
        encoder.string(text_decoration_id(*value))
    });
    encoder.specified("text-align", &style.text_align, |encoder, value| {
        encoder.string(text_align_id(*value))
    });
    encoder.specified("white-space", &style.white_space, |encoder, value| {
        encoder.string(white_space_id(*value))
    });
    encoder.specified("wrap", &style.wrap, |encoder, value| {
        encoder.string(wrap_mode_id(*value))
    });
}

fn encode_canvas(encoder: &mut CanonicalEncoder, canvas: &CanvasSpec) {
    encoder.field("base-specified", |encoder| {
        encoder.boolean(canvas.has_explicit_base())
    });
    encoder.field("base", |encoder| {
        encode_canvas_paint(encoder, canvas.base())
    });
    encoder.field("layers", |encoder| {
        encoder.sequence_len(canvas.layers().len());
        for layer in canvas.layers() {
            encoder.field("layer", |encoder| {
                encoder.field("paint", |encoder| {
                    encode_canvas_paint(encoder, layer.paint())
                });
                encoder.field("opacity", |encoder| encoder.f32(layer.opacity()));
                encoder.field("blend-mode", |encoder| {
                    encoder.string(layer.blend_mode().as_svg())
                });
                let (offset_x, offset_y) = layer.offset();
                encoder.field("offset-x", |encoder| encoder.f32(offset_x));
                encoder.field("offset-y", |encoder| encoder.f32(offset_y));
            });
        }
    });
    encoder.field("bleed", |encoder| encode_insets(encoder, canvas.bleed()));
}

fn encode_canvas_paint(encoder: &mut CanonicalEncoder, paint: &CanvasPaint) {
    match paint {
        CanvasPaint::Transparent => {
            encoder.field("kind", |encoder| encoder.string("transparent"));
        }
        CanvasPaint::Solid(color) => {
            encoder.field("kind", |encoder| encoder.string("solid"));
            encoder.field("color", |encoder| encode_color(encoder, color));
        }
        CanvasPaint::LinearGradient(gradient) => {
            encoder.field("kind", |encoder| encoder.string("linear-gradient"));
            encoder.field("angle-degrees", |encoder| {
                encoder.f32(gradient.angle_degrees())
            });
            encoder.field("stops", |encoder| {
                encode_gradient_stops(encoder, gradient.stops())
            });
            if let Some(period_px) = gradient.repeating_period_px() {
                encoder.field("repetition", |encoder| {
                    encoder.field("kind", |encoder| encoder.string("repeating"));
                    encoder.field("period-px", |encoder| encoder.f32(period_px));
                });
            } else if let Some((width_px, height_px)) = gradient.tile_size_px() {
                encode_gradient_tile(encoder, width_px, height_px);
            }
        }
        CanvasPaint::RadialGradient(gradient) => {
            encoder.field("kind", |encoder| encoder.string("radial-gradient"));
            encoder.field("center-x", |encoder| {
                encode_length(encoder, gradient.center_x())
            });
            encoder.field("center-y", |encoder| {
                encode_length(encoder, gradient.center_y())
            });
            encoder.field("radius", |encoder| {
                encode_length(encoder, gradient.radius())
            });
            encoder.field("stops", |encoder| {
                encode_gradient_stops(encoder, gradient.stops())
            });
            if gradient.is_repeating() {
                encoder.field("repetition", |encoder| {
                    encoder.field("kind", |encoder| encoder.string("repeating"));
                });
            } else if let Some((width_px, height_px)) = gradient.tile_size_px() {
                encode_gradient_tile(encoder, width_px, height_px);
            }
        }
        CanvasPaint::Pattern(pattern) => {
            encoder.field("kind", |encoder| encoder.string("pattern"));
            encoder.field("pattern-kind", |encoder| {
                encoder.string(pattern_kind_id(pattern.kind()))
            });
            encoder.field("cell-width", |encoder| encoder.f32(pattern.cell_width()));
            encoder.field("cell-height", |encoder| encoder.f32(pattern.cell_height()));
            encoder.field("foreground", |encoder| {
                encode_color(encoder, pattern.foreground())
            });
            encoder.field("background", |encoder| match pattern.background() {
                Some(color) => {
                    encoder.byte(1);
                    encode_color(encoder, color);
                }
                None => encoder.byte(0),
            });
            encoder.field("angle-degrees", |encoder| {
                encoder.f32(pattern.angle_degrees())
            });
        }
    }
}

fn encode_gradient_tile(encoder: &mut CanonicalEncoder, width_px: f32, height_px: f32) {
    encoder.field("repetition", |encoder| {
        encoder.field("kind", |encoder| encoder.string("tiled"));
        encoder.field("width-px", |encoder| encoder.f32(width_px));
        encoder.field("height-px", |encoder| encoder.f32(height_px));
    });
}

fn encode_gradient_stops(encoder: &mut CanonicalEncoder, stops: &[super::GradientStop]) {
    encoder.sequence_len(stops.len());
    for stop in stops {
        encoder.field("stop", |encoder| {
            encoder.field("offset", |encoder| encoder.f32(stop.offset()));
            encoder.field("color", |encoder| encode_color(encoder, stop.color()));
        });
    }
}

fn encode_length(encoder: &mut CanonicalEncoder, length: ThemeLength) {
    match length {
        ThemeLength::Px(value) => {
            encoder.field("unit", |encoder| encoder.string("px"));
            encoder.field("value", |encoder| encoder.f32(value));
        }
        ThemeLength::Percent(value) => {
            encoder.field("unit", |encoder| encoder.string("percent"));
            encoder.field("value", |encoder| encoder.f32(value));
        }
    }
}

fn encode_insets(encoder: &mut CanonicalEncoder, insets: InsetsPx) {
    encoder.field("top", |encoder| encoder.f32(insets.top));
    encoder.field("right", |encoder| encoder.f32(insets.right));
    encoder.field("bottom", |encoder| encoder.f32(insets.bottom));
    encoder.field("left", |encoder| encoder.f32(insets.left));
}

fn encode_color(encoder: &mut CanonicalEncoder, color: &ThemeColorValue) {
    encoder.string(&color.as_css());
}

fn encode_effects(encoder: &mut CanonicalEncoder, effects: &DiagramEffectSet) {
    let mut graphs = effects.graphs().iter().collect::<Vec<_>>();
    graphs.sort_by_key(|graph| graph.id());
    encoder.field("graphs", |encoder| {
        encoder.sequence_len(graphs.len());
        for graph in graphs {
            encoder.field("graph", |encoder| {
                encoder.field("id", |encoder| encoder.string(graph.id()));
                // The historical default retains its identity; explicit sRGB is a semantic input.
                if graph.color_space() != super::EffectColorSpace::LinearRgb {
                    encoder.field("color-space", |encoder| encoder.string("srgb"));
                }
                encoder.field("primitives", |encoder| {
                    encoder.sequence_len(graph.primitives().len());
                    for primitive in graph.primitives() {
                        encoder.field("primitive", |encoder| {
                            encode_effect_primitive(encoder, primitive)
                        });
                    }
                });
            });
        }
    });

    let mut bindings = effects.bindings().iter().collect::<Vec<_>>();
    bindings.sort_by_key(|binding| (binding.target().id(), binding.effect_id()));
    encoder.field("bindings", |encoder| {
        encoder.sequence_len(bindings.len());
        for binding in bindings {
            encoder.field("binding", |encoder| {
                encoder.field("target", |encoder| encoder.string(binding.target().id()));
                encoder.field("effect-id", |encoder| encoder.string(binding.effect_id()));
            });
        }
    });
}

fn encode_effect_primitive(encoder: &mut CanonicalEncoder, primitive: &EffectPrimitive) {
    match primitive {
        EffectPrimitive::DropShadow {
            input,
            offset_x,
            offset_y,
            blur_radius,
            spread,
            color,
        } => {
            encoder.field("kind", |encoder| encoder.string("drop-shadow"));
            encoder.field("input", |encoder| encoder.string(effect_input_id(*input)));
            encoder.field("offset-x", |encoder| encoder.f32(*offset_x));
            encoder.field("offset-y", |encoder| encoder.f32(*offset_y));
            encoder.field("blur-radius", |encoder| encoder.f32(*blur_radius));
            encoder.field("spread", |encoder| encoder.f32(*spread));
            encoder.field("color", |encoder| encode_color(encoder, color));
        }
        EffectPrimitive::GaussianBlur {
            input,
            std_deviation,
        } => {
            encoder.field("kind", |encoder| encoder.string("gaussian-blur"));
            encoder.field("input", |encoder| encoder.string(effect_input_id(*input)));
            encoder.field("std-deviation", |encoder| encoder.f32(*std_deviation));
        }
        EffectPrimitive::ColorMatrix { input, values } => {
            encoder.field("kind", |encoder| encoder.string("color-matrix"));
            encoder.field("input", |encoder| encoder.string(effect_input_id(*input)));
            encoder.field("values", |encoder| {
                encoder.sequence_len(values.len());
                for value in values {
                    encoder.f32(*value);
                }
            });
        }
        EffectPrimitive::Turbulence {
            input,
            base_frequency_x,
            base_frequency_y,
            octaves,
            seed,
        } => {
            encoder.field("kind", |encoder| encoder.string("turbulence"));
            encoder.field("input", |encoder| encoder.string(effect_input_id(*input)));
            encoder.field("base-frequency-x", |encoder| encoder.f32(*base_frequency_x));
            encoder.field("base-frequency-y", |encoder| encoder.f32(*base_frequency_y));
            encoder.field("octaves", |encoder| encoder.u64(u64::from(*octaves)));
            encoder.field("seed", |encoder| encoder.i64(i64::from(*seed)));
        }
        EffectPrimitive::Displacement {
            input,
            map_input,
            scale,
        } => {
            encoder.field("kind", |encoder| encoder.string("displacement"));
            encoder.field("input", |encoder| encoder.string(effect_input_id(*input)));
            encoder.field("map-input", |encoder| {
                encoder.string(effect_input_id(*map_input))
            });
            encoder.field("scale", |encoder| encoder.f32(*scale));
        }
    }
}

fn pattern_kind_id(kind: PatternKind) -> &'static str {
    match kind {
        PatternKind::Dots => "dots",
        PatternKind::Grid => "grid",
        PatternKind::Stripes => "stripes",
    }
}

fn effect_input_id(input: EffectInput) -> &'static str {
    match input {
        EffectInput::SourceGraphic => "source-graphic",
        EffectInput::Previous => "previous",
    }
}

fn stroke_linecap_id(value: StrokeLineCap) -> &'static str {
    value.id()
}

fn stroke_linejoin_id(value: StrokeLineJoin) -> &'static str {
    value.id()
}

fn font_style_id(value: FontStyle) -> &'static str {
    value.id()
}

fn text_transform_id(value: TextTransform) -> &'static str {
    match value {
        TextTransform::None => "none",
        TextTransform::Uppercase => "uppercase",
        TextTransform::Lowercase => "lowercase",
        TextTransform::Capitalize => "capitalize",
    }
}

fn text_decoration_id(value: TextDecoration) -> &'static str {
    match value {
        TextDecoration::None => "none",
        TextDecoration::Underline => "underline",
        TextDecoration::Overline => "overline",
        TextDecoration::LineThrough => "line-through",
    }
}

fn text_align_id(value: TextAlign) -> &'static str {
    match value {
        TextAlign::Start => "start",
        TextAlign::Center => "center",
        TextAlign::End => "end",
    }
}

fn white_space_id(value: WhiteSpace) -> &'static str {
    match value {
        WhiteSpace::Normal => "normal",
        WhiteSpace::Pre => "pre",
        WhiteSpace::NoWrap => "nowrap",
        WhiteSpace::PreWrap => "pre-wrap",
        WhiteSpace::PreLine => "pre-line",
    }
}

fn wrap_mode_id(value: WrapMode) -> &'static str {
    match value {
        WrapMode::Normal => "normal",
        WrapMode::BreakWord => "break-word",
        WrapMode::Anywhere => "anywhere",
    }
}

struct CanonicalEncoder {
    hasher: Sha256,
}

impl CanonicalEncoder {
    fn new(domain: &[u8]) -> Self {
        let mut encoder = Self {
            hasher: Sha256::new(),
        };
        encoder.bytes(domain);
        encoder
    }

    fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    fn field(&mut self, name: &str, encode: impl FnOnce(&mut Self)) {
        self.string(name);
        encode(self);
    }

    fn option_string(&mut self, name: &str, value: Option<&str>) {
        self.field(name, |encoder| match value {
            Some(value) => {
                encoder.byte(1);
                encoder.string(value);
            }
            None => encoder.byte(0),
        });
    }

    fn specified<T>(
        &mut self,
        name: &str,
        value: &Specified<T>,
        encode: impl FnOnce(&mut Self, &T),
    ) {
        self.field(name, |encoder| match value {
            Specified::Unspecified => encoder.byte(0),
            Specified::Clear => encoder.byte(1),
            Specified::Value(value) => {
                encoder.byte(2);
                encode(encoder, value);
            }
        });
    }

    fn specified_f32(&mut self, name: &str, value: &Specified<f32>) {
        self.specified(name, value, |encoder, value| encoder.f32(*value));
    }

    fn sequence_len(&mut self, value: usize) {
        self.usize(value);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(u64::try_from(value.len()).expect("recipe field length fits in u64"));
        self.hasher.update(value);
    }

    fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn byte(&mut self, value: u8) {
        self.hasher.update([value]);
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).expect("recipe collection length fits in u64"));
    }

    fn u64(&mut self, value: u64) {
        self.hasher.update(value.to_be_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.hasher.update(value.to_be_bytes());
    }

    fn f32(&mut self, value: f32) {
        let bits = if value == 0.0 { 0 } else { value.to_bits() };
        self.hasher.update(bits.to_be_bytes());
    }

    fn f64(&mut self, value: f64) {
        let bits = if value == 0.0 { 0 } else { value.to_bits() };
        self.hasher.update(bits.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        BlendMode, CanvasLayer, DiagramEffectSet, DiagramThemeCompiler, EffectBinding, EffectGraph,
        FontAssetSpec, FontCatalogSpec, GradientStop, LinearGradient, MermaidThemeCompatibility,
        OrdinalPalette, PatternSpec, RadialGradient, StrokeLineCap, StrokeLineJoin, ThemeAssets,
        ThemeCapability, ThemeCompileValidationError, ThemeRule, ThemeRuleSet, ThemeTarget,
        ThemeVariant, TypographySpec,
    };

    fn compile(spec: DiagramThemeSpec) -> super::super::ThemeRecipeFingerprint {
        DiagramThemeCompiler::new()
            .compile(spec)
            .expect("canonical recipe fixture should compile")
            .recipe_fingerprint()
    }

    fn color(value: &str) -> ThemeColorValue {
        ThemeColorValue::parse(value).expect("valid fixture color")
    }

    fn blur_graph(id: &str, deviation: f32) -> EffectGraph {
        EffectGraph::new(
            id,
            [EffectPrimitive::GaussianBlur {
                input: EffectInput::SourceGraphic,
                std_deviation: deviation,
            }],
        )
        .expect("valid effect graph")
    }

    fn assert_recipe_field_changes(field: &str, left: DiagramThemeSpec, right: DiagramThemeSpec) {
        assert_ne!(
            compile(left),
            compile(right),
            "semantic field {field} must affect recipe identity"
        );
    }

    fn with_default_text_style(style: TextStyle) -> DiagramThemeSpec {
        DiagramThemeSpec::new().with_typography(TypographySpec::default().with_default(style))
    }

    fn with_rule(rule: ThemeRule) -> DiagramThemeSpec {
        DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule))
    }

    fn node_rule(style: ThemeStylePatch) -> ThemeRule {
        ThemeRule::new(ThemeTarget::Node, style)
    }

    fn with_canvas(canvas: CanvasSpec) -> DiagramThemeSpec {
        DiagramThemeSpec::new().with_canvas(canvas)
    }

    fn gradient_stops(first_offset: f32, first_color: &str) -> [GradientStop; 2] {
        [
            GradientStop::new(first_offset, color(first_color)).unwrap(),
            GradientStop::new(1.0, color("#f8fafc")).unwrap(),
        ]
    }

    fn linear_paint(angle: f32, first_offset: f32, first_color: &str) -> CanvasPaint {
        CanvasPaint::LinearGradient(
            LinearGradient::new(angle, gradient_stops(first_offset, first_color)).unwrap(),
        )
    }

    fn radial_paint(
        center_x: ThemeLength,
        center_y: ThemeLength,
        radius: ThemeLength,
        first_offset: f32,
        first_color: &str,
    ) -> CanvasPaint {
        CanvasPaint::RadialGradient(
            RadialGradient::new(
                center_x,
                center_y,
                radius,
                gradient_stops(first_offset, first_color),
            )
            .unwrap(),
        )
    }

    fn pattern_paint(
        kind: PatternKind,
        cell_width: f32,
        cell_height: f32,
        foreground: &str,
        background: &str,
        angle: f32,
    ) -> CanvasPaint {
        CanvasPaint::Pattern(
            PatternSpec::new(kind, cell_width, cell_height, color(foreground))
                .unwrap()
                .with_background(color(background))
                .with_angle_degrees(angle)
                .unwrap(),
        )
    }

    fn effect_graph(
        id: &str,
        primitives: impl IntoIterator<Item = EffectPrimitive>,
    ) -> EffectGraph {
        EffectGraph::new(id, primitives).expect("valid canonical effect fixture")
    }

    fn with_effect_graph(graph: EffectGraph) -> DiagramThemeSpec {
        DiagramThemeSpec::new().with_effects(
            DiagramEffectSet::default()
                .with_graph(graph)
                .expect("unique effect graph"),
        )
    }

    #[test]
    fn every_explicit_typography_default_changes_recipe_identity() {
        let baseline = TextStyle::default();
        let cases = [
            baseline
                .clone()
                .with_font_stack(baseline.font_stack().clone()),
            baseline
                .clone()
                .with_font_size_px(baseline.font_size_px())
                .unwrap(),
            baseline
                .clone()
                .with_font_weight(baseline.font_weight())
                .unwrap(),
            baseline.clone().with_font_style(baseline.font_style()),
            baseline
                .clone()
                .with_line_height(baseline.line_height())
                .unwrap(),
            baseline
                .clone()
                .with_letter_spacing_px(baseline.letter_spacing_px())
                .unwrap(),
            baseline
                .clone()
                .with_word_spacing_px(baseline.word_spacing_px())
                .unwrap(),
            baseline.clone().with_transform(baseline.transform()),
            baseline.clone().with_decoration(baseline.decoration()),
            baseline.clone().with_text_align(baseline.text_align()),
            baseline.clone().with_white_space(baseline.white_space()),
            baseline.clone().with_wrap(baseline.wrap()),
        ];
        for (property, style) in crate::diagram_theme::ThemeTypographyProperty::ALL
            .iter()
            .zip(cases)
        {
            assert_eq!(
                style.specified_properties().collect::<Vec<_>>(),
                [*property]
            );
            assert_recipe_field_changes(
                property.id(),
                with_default_text_style(baseline.clone()),
                with_default_text_style(style),
            );
        }
    }

    #[test]
    fn explicit_default_typography_is_distinct_from_omission_in_rust_and_wire() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, FamilyThemeMechanism, ThemeTypographyProperty,
        };
        let compiler = DiagramThemeCompiler::new();
        let omitted = compiler.compile(DiagramThemeSpec::new()).unwrap();
        let explicit = compiler
            .compile(with_default_text_style(
                TextStyle::default().with_font_size_px(16.0).unwrap(),
            ))
            .unwrap();
        let wire = compiler
            .compile_spec_wire(
                serde_json::from_value(serde_json::json!({
                    "typography": { "default": { "font_size_px": 16.0 } }
                }))
                .unwrap(),
            )
            .unwrap();
        assert_ne!(omitted.recipe_fingerprint(), explicit.recipe_fingerprint());
        assert_eq!(explicit.recipe_fingerprint(), wire.recipe_fingerprint());
        for family in [crate::DiagramFamilyId::BLOCK, crate::DiagramFamilyId::PIE] {
            assert!(
                !omitted
                    .resolve(family)
                    .family_mechanism_routes()
                    .iter()
                    .any(|route| route.mechanism()
                        == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize))
            );
            assert!(
                explicit
                    .resolve(family)
                    .family_mechanism_routes()
                    .iter()
                    .any(|route| route.mechanism()
                        == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize))
            );
        }
    }

    #[test]
    fn canonical_recipe_ignores_semantically_unordered_builder_order() {
        let left_mermaid = MermaidThemeCompatibility::default()
            .with_variable("primaryColor", "#112233")
            .unwrap()
            .with_variable("fontSize", 18.0)
            .unwrap();
        let right_mermaid = MermaidThemeCompatibility::default()
            .with_variable("fontSize", 18.0)
            .unwrap()
            .with_variable("primaryColor", "#112233")
            .unwrap();

        let node_palette = OrdinalPalette::new([color("#ef4444"), color("#22c55e")]).unwrap();
        let edge_palette = OrdinalPalette::new([color("#3b82f6"), color("#a855f7")]).unwrap();
        let left_rules = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Node, node_palette.clone())
            .with_ordinal_palette(ThemeTarget::Edge, edge_palette.clone());
        let right_rules = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Edge, edge_palette)
            .with_ordinal_palette(ThemeTarget::Node, node_palette);

        let graph_a = blur_graph("a", 1.0);
        let graph_b = blur_graph("b", 2.0);
        let left_effects = DiagramEffectSet::default()
            .with_graph(graph_a.clone())
            .unwrap()
            .with_graph(graph_b.clone())
            .unwrap();
        let right_effects = DiagramEffectSet::default()
            .with_graph(graph_b)
            .unwrap()
            .with_graph(graph_a)
            .unwrap();

        let left = DiagramThemeSpec::new()
            .with_mermaid_compatibility(left_mermaid)
            .with_styles(left_rules)
            .with_effects(left_effects);
        let right = DiagramThemeSpec::new()
            .with_mermaid_compatibility(right_mermaid)
            .with_styles(right_rules)
            .with_effects(right_effects);

        assert_eq!(compile(left), compile(right));
    }

    #[test]
    fn public_spec_fields_fully_determine_recipe_identity() {
        let source = DiagramThemeSpec::new()
            .with_typography(TypographySpec::default().with_default(TextStyle::default()))
            .with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::Solid(color("#e2e8f0"))),
            )))
            .with_canvas(CanvasSpec::default().with_base(CanvasPaint::Solid(color("#ffffff"))));
        let rebuilt = DiagramThemeSpec::new()
            .with_mermaid_compatibility(source.mermaid().clone())
            .with_typography(source.typography().clone())
            .with_styles(source.styles().clone())
            .with_canvas(source.canvas().clone())
            .with_effects(source.effects().clone())
            .with_assets(source.assets().clone())
            .with_requirements(source.requirements().clone());

        assert_eq!(compile(source), compile(rebuilt));
    }

    #[test]
    fn canonical_recipe_preserves_rule_and_layer_order() {
        let red = ThemeStylePatch::default().with_fill(CanvasPaint::Solid(color("#ef4444")));
        let blue = ThemeStylePatch::default().with_fill(CanvasPaint::Solid(color("#3b82f6")));
        let red_then_blue = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, red.clone()))
            .with_rule(ThemeRule::new(ThemeTarget::Node, blue.clone()));
        let blue_then_red = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(ThemeTarget::Node, blue))
            .with_rule(ThemeRule::new(ThemeTarget::Node, red));
        assert_ne!(
            compile(DiagramThemeSpec::new().with_styles(red_then_blue)),
            compile(DiagramThemeSpec::new().with_styles(blue_then_red))
        );

        let layer_a = CanvasLayer::new(CanvasPaint::Solid(color("#111827")));
        let layer_b = CanvasLayer::new(CanvasPaint::Solid(color("#f8fafc")));
        let first = CanvasSpec::default()
            .with_layer(layer_a.clone())
            .unwrap()
            .with_layer(layer_b.clone())
            .unwrap();
        let second = CanvasSpec::default()
            .with_layer(layer_b)
            .unwrap()
            .with_layer(layer_a)
            .unwrap();
        assert_ne!(
            compile(DiagramThemeSpec::new().with_canvas(first)),
            compile(DiagramThemeSpec::new().with_canvas(second))
        );
    }

    #[test]
    fn canonical_recipe_normalizes_negative_zero() {
        let positive = CanvasSpec::default()
            .with_layer(
                CanvasLayer::new(CanvasPaint::Transparent)
                    .with_offset(0.0, 0.0)
                    .unwrap(),
            )
            .unwrap();
        let negative = CanvasSpec::default()
            .with_layer(
                CanvasLayer::new(CanvasPaint::Transparent)
                    .with_offset(-0.0, -0.0)
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(
            compile(DiagramThemeSpec::new().with_canvas(positive)),
            compile(DiagramThemeSpec::new().with_canvas(negative))
        );
    }

    #[test]
    fn every_mermaid_compatibility_field_changes_recipe_identity() {
        assert_recipe_field_changes(
            "mermaid.theme",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_theme("base")
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_theme("dark")
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "mermaid.dark_mode",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_dark_mode(false)
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_dark_mode(true)
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "mermaid.variable.name",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("primaryColor", "#112233")
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("secondaryColor", "#112233")
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "mermaid.variable.string",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("primaryColor", "#112233")
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("primaryColor", "#223344")
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "mermaid.variable.number",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("fontSize", 17.0)
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("fontSize", 18.0)
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "mermaid.variable.boolean",
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("darkMode", false)
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_variable("darkMode", true)
                    .unwrap(),
            ),
        );
    }

    #[test]
    fn every_text_style_field_changes_recipe_identity() {
        let cases = [
            (
                "typography.font_stack",
                TextStyle::default()
                    .with_font_stack(FontStack::single("Inter").expect("valid font stack")),
                TextStyle::default()
                    .with_font_stack(FontStack::single("Fira Sans").expect("valid font stack")),
            ),
            (
                "typography.font_size_px",
                TextStyle::default().with_font_size_px(17.0).unwrap(),
                TextStyle::default().with_font_size_px(18.0).unwrap(),
            ),
            (
                "typography.font_weight",
                TextStyle::default().with_font_weight(500).unwrap(),
                TextStyle::default().with_font_weight(600).unwrap(),
            ),
            (
                "typography.font_style",
                TextStyle::default().with_font_style(FontStyle::Italic),
                TextStyle::default().with_font_style(FontStyle::Oblique),
            ),
            (
                "typography.line_height",
                TextStyle::default()
                    .with_line_height(LineHeight::Multiplier(1.2))
                    .unwrap(),
                TextStyle::default()
                    .with_line_height(LineHeight::Multiplier(1.3))
                    .unwrap(),
            ),
            (
                "typography.letter_spacing_px",
                TextStyle::default().with_letter_spacing_px(1.0).unwrap(),
                TextStyle::default().with_letter_spacing_px(2.0).unwrap(),
            ),
            (
                "typography.word_spacing_px",
                TextStyle::default().with_word_spacing_px(1.0).unwrap(),
                TextStyle::default().with_word_spacing_px(2.0).unwrap(),
            ),
            (
                "typography.transform",
                TextStyle::default().with_transform(TextTransform::Uppercase),
                TextStyle::default().with_transform(TextTransform::Lowercase),
            ),
            (
                "typography.decoration",
                TextStyle::default().with_decoration(TextDecoration::Underline),
                TextStyle::default().with_decoration(TextDecoration::Overline),
            ),
            (
                "typography.text_align",
                TextStyle::default().with_text_align(TextAlign::Center),
                TextStyle::default().with_text_align(TextAlign::End),
            ),
            (
                "typography.white_space",
                TextStyle::default().with_white_space(WhiteSpace::Pre),
                TextStyle::default().with_white_space(WhiteSpace::NoWrap),
            ),
            (
                "typography.wrap",
                TextStyle::default().with_wrap(WrapMode::BreakWord),
                TextStyle::default().with_wrap(WrapMode::Anywhere),
            ),
        ];

        for (field, left, right) in cases {
            assert_recipe_field_changes(
                field,
                with_default_text_style(left),
                with_default_text_style(right),
            );
        }

        let flowchart = TypographySpec::default().with_family_style(
            DiagramFamilyId::FLOWCHART,
            TextStyle::default().with_font_size_px(17.0).unwrap(),
        );
        let sequence = TypographySpec::default().with_family_style(
            DiagramFamilyId::SEQUENCE,
            TextStyle::default().with_font_size_px(17.0).unwrap(),
        );
        assert_recipe_field_changes(
            "typography.family_override.family",
            DiagramThemeSpec::new().with_typography(flowchart),
            DiagramThemeSpec::new().with_typography(sequence),
        );
    }

    #[test]
    fn every_semantic_rule_field_changes_recipe_identity() {
        let red = CanvasPaint::Solid(color("#ef4444"));
        let blue = CanvasPaint::Solid(color("#3b82f6"));
        let mut style_pairs = Vec::new();

        style_pairs.push((
            "style.fill",
            ThemeStylePatch::default().with_fill(red.clone()),
            ThemeStylePatch::default().with_fill(blue.clone()),
        ));

        let mut left = ThemeStylePatch::default();
        left.paint.opacity = Specified::Value(0.4);
        let mut right = ThemeStylePatch::default();
        right.paint.opacity = Specified::Value(0.6);
        style_pairs.push(("style.opacity", left, right));

        let mut left = ThemeStylePatch::default();
        left.paint.fill_opacity = Specified::Value(0.4);
        let mut right = ThemeStylePatch::default();
        right.paint.fill_opacity = Specified::Value(0.6);
        style_pairs.push(("style.fill_opacity", left, right));

        style_pairs.push((
            "style.stroke.paint",
            ThemeStylePatch::default().with_stroke(red.clone()),
            ThemeStylePatch::default().with_stroke(blue.clone()),
        ));
        style_pairs.push((
            "style.stroke.width",
            ThemeStylePatch::default().with_stroke_width(1.0).unwrap(),
            ThemeStylePatch::default().with_stroke_width(2.0).unwrap(),
        ));
        style_pairs.push((
            "style.stroke.dasharray",
            ThemeStylePatch::default()
                .with_stroke_dasharray([1.0, 2.0])
                .unwrap(),
            ThemeStylePatch::default()
                .with_stroke_dasharray([2.0, 1.0])
                .unwrap(),
        ));

        let mut left = ThemeStylePatch::default();
        left.stroke.linecap = Specified::Value(StrokeLineCap::Round);
        let mut right = ThemeStylePatch::default();
        right.stroke.linecap = Specified::Value(StrokeLineCap::Square);
        style_pairs.push(("style.stroke.linecap", left, right));

        let mut left = ThemeStylePatch::default();
        left.stroke.linejoin = Specified::Value(StrokeLineJoin::Round);
        let mut right = ThemeStylePatch::default();
        right.stroke.linejoin = Specified::Value(StrokeLineJoin::Bevel);
        style_pairs.push(("style.stroke.linejoin", left, right));

        let mut left = ThemeStylePatch::default();
        left.stroke.stroke_opacity = Specified::Value(0.4);
        let mut right = ThemeStylePatch::default();
        right.stroke.stroke_opacity = Specified::Value(0.6);
        style_pairs.push(("style.stroke.opacity", left, right));

        let mut left = ThemeStylePatch::default();
        left.geometry.radius = Specified::Value(2.0);
        let mut right = ThemeStylePatch::default();
        right.geometry.radius = Specified::Value(4.0);
        style_pairs.push(("style.geometry.radius", left, right));

        style_pairs.push((
            "style.spacing.padding",
            ThemeStylePatch::default().with_padding(InsetsPx {
                top: 1.0,
                right: 2.0,
                bottom: 3.0,
                left: 4.0,
            }),
            ThemeStylePatch::default().with_padding(InsetsPx {
                top: 1.0,
                right: 2.0,
                bottom: 3.0,
                left: 5.0,
            }),
        ));

        macro_rules! typography_pair {
            ($field:literal, $member:ident, $left:expr, $right:expr) => {{
                let mut left = ThemeStylePatch::default();
                left.typography.$member = Specified::Value($left);
                let mut right = ThemeStylePatch::default();
                right.typography.$member = Specified::Value($right);
                style_pairs.push(($field, left, right));
            }};
        }

        typography_pair!(
            "style.typography.font_stack",
            font_stack,
            FontStack::single("Inter").unwrap(),
            FontStack::single("Fira Sans").unwrap()
        );
        typography_pair!("style.typography.font_size", font_size_px, 17.0, 18.0);
        typography_pair!("style.typography.font_weight", font_weight, 500, 600);
        typography_pair!(
            "style.typography.font_style",
            font_style,
            FontStyle::Italic,
            FontStyle::Oblique
        );
        typography_pair!(
            "style.typography.line_height",
            line_height,
            LineHeight::Multiplier(1.2),
            LineHeight::Multiplier(1.3)
        );
        typography_pair!(
            "style.typography.letter_spacing",
            letter_spacing_px,
            1.0,
            2.0
        );
        typography_pair!("style.typography.word_spacing", word_spacing_px, 1.0, 2.0);
        typography_pair!(
            "style.typography.transform",
            transform,
            TextTransform::Uppercase,
            TextTransform::Lowercase
        );
        typography_pair!(
            "style.typography.decoration",
            decoration,
            TextDecoration::Underline,
            TextDecoration::Overline
        );
        typography_pair!(
            "style.typography.text_align",
            text_align,
            TextAlign::Center,
            TextAlign::End
        );
        typography_pair!(
            "style.typography.white_space",
            white_space,
            WhiteSpace::Pre,
            WhiteSpace::NoWrap
        );
        typography_pair!(
            "style.typography.wrap",
            wrap,
            WrapMode::BreakWord,
            WrapMode::Anywhere
        );

        let mut clear_opacity = ThemeStylePatch::default();
        clear_opacity.paint.opacity = Specified::Clear;
        let mut value_opacity = ThemeStylePatch::default();
        value_opacity.paint.opacity = Specified::Value(0.5);
        style_pairs.push(("style.specified_discriminant", clear_opacity, value_opacity));

        for (field, left, right) in style_pairs {
            assert_recipe_field_changes(
                field,
                with_rule(node_rule(left)),
                with_rule(node_rule(right)),
            );
        }

        let effects = DiagramEffectSet::default()
            .with_graph(blur_graph("soft", 1.0))
            .unwrap()
            .with_graph(blur_graph("rough", 2.0))
            .unwrap();
        assert_recipe_field_changes(
            "style.effect",
            DiagramThemeSpec::new()
                .with_effects(effects.clone())
                .with_styles(ThemeRuleSet::default().with_rule(node_rule(
                    ThemeStylePatch::default().with_effect("soft").unwrap(),
                ))),
            DiagramThemeSpec::new().with_effects(effects).with_styles(
                ThemeRuleSet::default().with_rule(node_rule(
                    ThemeStylePatch::default().with_effect("rough").unwrap(),
                )),
            ),
        );

        let style = ThemeStylePatch::default().with_fill(red);
        assert_recipe_field_changes(
            "rule.target",
            with_rule(ThemeRule::new(ThemeTarget::Node, style.clone())),
            with_rule(ThemeRule::new(ThemeTarget::Edge, style.clone())),
        );
        assert_recipe_field_changes(
            "rule.family",
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .for_family(DiagramFamilyId::FLOWCHART),
            ),
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .for_family(DiagramFamilyId::SWIMLANE),
            ),
        );
        assert_recipe_field_changes(
            "rule.variant",
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_variant(ThemeVariant::Primary),
            ),
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_variant(ThemeVariant::Secondary),
            ),
        );
        assert_recipe_field_changes(
            "rule.ordinal.exact",
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            ),
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_ordinal(OrdinalSelector::exact(2).unwrap()),
            ),
        );
        assert_recipe_field_changes(
            "rule.ordinal.cycle.period",
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_ordinal(OrdinalSelector::cycle(2, 0).unwrap()),
            ),
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_ordinal(OrdinalSelector::cycle(3, 0).unwrap()),
            ),
        );
        assert_recipe_field_changes(
            "rule.ordinal.cycle.offset",
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style.clone())
                    .with_ordinal(OrdinalSelector::cycle(3, 0).unwrap()),
            ),
            with_rule(
                ThemeRule::new(ThemeTarget::Node, style)
                    .with_ordinal(OrdinalSelector::cycle(3, 1).unwrap()),
            ),
        );

        assert_recipe_field_changes(
            "ordinal_palette.color",
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_ordinal_palette(
                ThemeTarget::Node,
                OrdinalPalette::new([color("#ef4444"), color("#22c55e")]).unwrap(),
            )),
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_ordinal_palette(
                ThemeTarget::Node,
                OrdinalPalette::new([color("#ef4444"), color("#3b82f6")]).unwrap(),
            )),
        );
    }

    #[test]
    fn every_canvas_field_changes_recipe_identity() {
        assert_recipe_field_changes(
            "canvas.base.specified",
            with_canvas(CanvasSpec::default()),
            with_canvas(CanvasSpec::transparent()),
        );
        let mut cases = Vec::new();
        cases.push((
            "canvas.solid.color",
            CanvasSpec::default().with_base(CanvasPaint::Solid(color("#111827"))),
            CanvasSpec::default().with_base(CanvasPaint::Solid(color("#1f2937"))),
        ));
        cases.push((
            "canvas.linear.angle",
            CanvasSpec::default().with_base(linear_paint(30.0, 0.0, "#111827")),
            CanvasSpec::default().with_base(linear_paint(45.0, 0.0, "#111827")),
        ));
        cases.push((
            "canvas.linear.stop.offset",
            CanvasSpec::default().with_base(linear_paint(30.0, 0.0, "#111827")),
            CanvasSpec::default().with_base(linear_paint(30.0, 0.1, "#111827")),
        ));
        cases.push((
            "canvas.linear.stop.color",
            CanvasSpec::default().with_base(linear_paint(30.0, 0.0, "#111827")),
            CanvasSpec::default().with_base(linear_paint(30.0, 0.0, "#1f2937")),
        ));
        cases.push((
            "canvas.linear.repeating_period_px",
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_repeating_period_px(16.0)
                    .unwrap(),
            )),
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_repeating_period_px(24.0)
                    .unwrap(),
            )),
        ));
        cases.push((
            "canvas.linear.tile_width_px",
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_tile_px(16.0, 20.0)
                    .unwrap(),
            )),
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_tile_px(24.0, 20.0)
                    .unwrap(),
            )),
        ));
        cases.push((
            "canvas.linear.tile_height_px",
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_tile_px(16.0, 20.0)
                    .unwrap(),
            )),
            CanvasSpec::default().with_base(CanvasPaint::LinearGradient(
                LinearGradient::new(30.0, gradient_stops(0.0, "#111827"))
                    .unwrap()
                    .with_tile_px(16.0, 24.0)
                    .unwrap(),
            )),
        ));
        cases.push((
            "canvas.radial.center_x",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(40.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(45.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
        ));
        cases.push((
            "canvas.radial.center_y",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(40.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(45.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
        ));
        cases.push((
            "canvas.radial.radius",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(70.0),
                0.0,
                "#111827",
            )),
        ));
        cases.push((
            "canvas.radial.length_unit",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::px(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
        ));
        cases.push((
            "canvas.radial.stop",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.1,
                "#1f2937",
            )),
        ));
        cases.push((
            "canvas.radial.repeating",
            CanvasSpec::default().with_base(radial_paint(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(60.0),
                0.0,
                "#111827",
            )),
            CanvasSpec::default().with_base(CanvasPaint::RadialGradient(
                RadialGradient::new(
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(60.0),
                    gradient_stops(0.0, "#111827"),
                )
                .unwrap()
                .with_repeating()
                .unwrap(),
            )),
        ));
        cases.push((
            "canvas.radial.tile_size_px",
            CanvasSpec::default().with_base(CanvasPaint::RadialGradient(
                RadialGradient::new(
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(60.0),
                    gradient_stops(0.0, "#111827"),
                )
                .unwrap()
                .with_tile_px(16.0, 20.0)
                .unwrap(),
            )),
            CanvasSpec::default().with_base(CanvasPaint::RadialGradient(
                RadialGradient::new(
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(50.0),
                    ThemeLength::percent(60.0),
                    gradient_stops(0.0, "#111827"),
                )
                .unwrap()
                .with_tile_px(20.0, 20.0)
                .unwrap(),
            )),
        ));
        cases.push((
            "canvas.pattern.kind",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Grid,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
        ));
        cases.push((
            "canvas.pattern.cell_width",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                13.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
        ));
        cases.push((
            "canvas.pattern.cell_height",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                15.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
        ));
        cases.push((
            "canvas.pattern.foreground",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#1f2937",
                "#f8fafc",
                10.0,
            )),
        ));
        cases.push((
            "canvas.pattern.background",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#e2e8f0",
                10.0,
            )),
        ));
        cases.push((
            "canvas.pattern.angle",
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                10.0,
            )),
            CanvasSpec::default().with_base(pattern_paint(
                PatternKind::Dots,
                12.0,
                14.0,
                "#111827",
                "#f8fafc",
                20.0,
            )),
        ));

        let layer = |paint: CanvasPaint, opacity: f32, blend, offset_x, offset_y| {
            CanvasLayer::new(paint)
                .with_opacity(opacity)
                .unwrap()
                .with_blend_mode(blend)
                .with_offset(offset_x, offset_y)
                .unwrap()
        };
        cases.push((
            "canvas.layer.paint",
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#1f2937")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
        ));
        cases.push((
            "canvas.layer.opacity",
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.7,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
        ));
        cases.push((
            "canvas.layer.blend_mode",
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Screen,
                    1.0,
                    2.0,
                ))
                .unwrap(),
        ));
        cases.push((
            "canvas.layer.offset_x",
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    3.0,
                    2.0,
                ))
                .unwrap(),
        ));
        cases.push((
            "canvas.layer.offset_y",
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    2.0,
                ))
                .unwrap(),
            CanvasSpec::default()
                .with_layer(layer(
                    CanvasPaint::Solid(color("#111827")),
                    0.8,
                    BlendMode::Multiply,
                    1.0,
                    3.0,
                ))
                .unwrap(),
        ));

        for (field, left, right) in cases {
            assert_recipe_field_changes(field, with_canvas(left), with_canvas(right));
        }

        for (field, left, right) in [
            (
                "canvas.bleed.top",
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 4.0,
                },
                InsetsPx {
                    top: 5.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 4.0,
                },
            ),
            (
                "canvas.bleed.right",
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 4.0,
                },
                InsetsPx {
                    top: 1.0,
                    right: 5.0,
                    bottom: 3.0,
                    left: 4.0,
                },
            ),
            (
                "canvas.bleed.bottom",
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 4.0,
                },
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 5.0,
                    left: 4.0,
                },
            ),
            (
                "canvas.bleed.left",
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 4.0,
                },
                InsetsPx {
                    top: 1.0,
                    right: 2.0,
                    bottom: 3.0,
                    left: 5.0,
                },
            ),
        ] {
            assert_recipe_field_changes(
                field,
                with_canvas(CanvasSpec::default().with_bleed(left).unwrap()),
                with_canvas(CanvasSpec::default().with_bleed(right).unwrap()),
            );
        }
    }

    #[test]
    fn every_effect_field_changes_recipe_identity() {
        let blur = |deviation| EffectPrimitive::GaussianBlur {
            input: EffectInput::SourceGraphic,
            std_deviation: deviation,
        };
        let graph_spec = |id: &str, primitives: Vec<EffectPrimitive>| {
            with_effect_graph(effect_graph(id, primitives))
        };

        assert_recipe_field_changes(
            "effects.graph.id",
            graph_spec("soft-a", vec![blur(1.0)]),
            graph_spec("soft-b", vec![blur(1.0)]),
        );

        let shadow = |input, offset_x, offset_y, blur_radius, spread, color_value: &str| {
            EffectPrimitive::DropShadow {
                input,
                offset_x,
                offset_y,
                blur_radius,
                spread,
                color: color(color_value),
            }
        };
        let shadow_graph = |shadow| graph_spec("shadow", vec![blur(1.0), shadow]);
        assert_recipe_field_changes(
            "effects.drop_shadow.input",
            shadow_graph(shadow(
                EffectInput::SourceGraphic,
                1.0,
                2.0,
                3.0,
                0.0,
                "#111827",
            )),
            shadow_graph(shadow(EffectInput::Previous, 1.0, 2.0, 3.0, 0.0, "#111827")),
        );
        for (field, left, right) in [
            (
                "effects.drop_shadow.offset_x",
                (1.0, 2.0, 3.0, 0.0),
                (2.0, 2.0, 3.0, 0.0),
            ),
            (
                "effects.drop_shadow.offset_y",
                (1.0, 2.0, 3.0, 0.0),
                (1.0, 3.0, 3.0, 0.0),
            ),
            (
                "effects.drop_shadow.blur_radius",
                (1.0, 2.0, 3.0, 0.0),
                (1.0, 2.0, 4.0, 0.0),
            ),
            (
                "effects.drop_shadow.spread",
                (1.0, 2.0, 3.0, 0.0),
                (1.0, 2.0, 3.0, 1.0),
            ),
        ] {
            assert_recipe_field_changes(
                field,
                shadow_graph(shadow(
                    EffectInput::Previous,
                    left.0,
                    left.1,
                    left.2,
                    left.3,
                    "#111827",
                )),
                shadow_graph(shadow(
                    EffectInput::Previous,
                    right.0,
                    right.1,
                    right.2,
                    right.3,
                    "#111827",
                )),
            );
        }
        assert_recipe_field_changes(
            "effects.drop_shadow.color",
            shadow_graph(shadow(EffectInput::Previous, 1.0, 2.0, 3.0, 0.0, "#111827")),
            shadow_graph(shadow(EffectInput::Previous, 1.0, 2.0, 3.0, 0.0, "#1f2937")),
        );

        assert_recipe_field_changes(
            "effects.gaussian_blur.input",
            graph_spec("blur", vec![blur(1.0), blur(2.0)]),
            graph_spec(
                "blur",
                vec![
                    blur(1.0),
                    EffectPrimitive::GaussianBlur {
                        input: EffectInput::Previous,
                        std_deviation: 2.0,
                    },
                ],
            ),
        );
        assert_recipe_field_changes(
            "effects.gaussian_blur.std_deviation",
            graph_spec("blur", vec![blur(1.0)]),
            graph_spec("blur", vec![blur(2.0)]),
        );

        let mut matrix_a = [0.0; 20];
        matrix_a[0] = 1.0;
        let mut matrix_b = matrix_a;
        matrix_b[19] = 1.0;
        let matrix = |input, values| EffectPrimitive::ColorMatrix { input, values };
        assert_recipe_field_changes(
            "effects.color_matrix.input",
            graph_spec(
                "matrix",
                vec![blur(1.0), matrix(EffectInput::SourceGraphic, matrix_a)],
            ),
            graph_spec(
                "matrix",
                vec![blur(1.0), matrix(EffectInput::Previous, matrix_a)],
            ),
        );
        assert_recipe_field_changes(
            "effects.color_matrix.values",
            graph_spec("matrix", vec![matrix(EffectInput::SourceGraphic, matrix_a)]),
            graph_spec("matrix", vec![matrix(EffectInput::SourceGraphic, matrix_b)]),
        );

        let turbulence = |input, x, y, octaves, seed| EffectPrimitive::Turbulence {
            input,
            base_frequency_x: x,
            base_frequency_y: y,
            octaves,
            seed,
        };
        assert_recipe_field_changes(
            "effects.turbulence.input",
            graph_spec(
                "noise",
                vec![
                    blur(1.0),
                    turbulence(EffectInput::SourceGraphic, 0.1, 0.2, 2, 7),
                ],
            ),
            graph_spec(
                "noise",
                vec![blur(1.0), turbulence(EffectInput::Previous, 0.1, 0.2, 2, 7)],
            ),
        );
        for (field, left, right) in [
            (
                "effects.turbulence.base_frequency_x",
                (0.1, 0.2, 2, 7),
                (0.2, 0.2, 2, 7),
            ),
            (
                "effects.turbulence.base_frequency_y",
                (0.1, 0.2, 2, 7),
                (0.1, 0.3, 2, 7),
            ),
            (
                "effects.turbulence.octaves",
                (0.1, 0.2, 2, 7),
                (0.1, 0.2, 3, 7),
            ),
            (
                "effects.turbulence.seed",
                (0.1, 0.2, 2, 7),
                (0.1, 0.2, 2, 8),
            ),
        ] {
            assert_recipe_field_changes(
                field,
                graph_spec(
                    "noise",
                    vec![turbulence(
                        EffectInput::SourceGraphic,
                        left.0,
                        left.1,
                        left.2,
                        left.3,
                    )],
                ),
                graph_spec(
                    "noise",
                    vec![turbulence(
                        EffectInput::SourceGraphic,
                        right.0,
                        right.1,
                        right.2,
                        right.3,
                    )],
                ),
            );
        }

        let displacement = |input, map_input, scale| EffectPrimitive::Displacement {
            input,
            map_input,
            scale,
        };
        assert_recipe_field_changes(
            "effects.displacement.input",
            graph_spec(
                "rough",
                vec![
                    blur(1.0),
                    displacement(EffectInput::SourceGraphic, EffectInput::SourceGraphic, 4.0),
                ],
            ),
            graph_spec(
                "rough",
                vec![
                    blur(1.0),
                    displacement(EffectInput::Previous, EffectInput::SourceGraphic, 4.0),
                ],
            ),
        );
        assert_recipe_field_changes(
            "effects.displacement.map_input",
            graph_spec(
                "rough",
                vec![
                    blur(1.0),
                    displacement(EffectInput::SourceGraphic, EffectInput::SourceGraphic, 4.0),
                ],
            ),
            graph_spec(
                "rough",
                vec![
                    blur(1.0),
                    displacement(EffectInput::SourceGraphic, EffectInput::Previous, 4.0),
                ],
            ),
        );
        assert_recipe_field_changes(
            "effects.displacement.scale",
            graph_spec(
                "rough",
                vec![displacement(
                    EffectInput::SourceGraphic,
                    EffectInput::SourceGraphic,
                    4.0,
                )],
            ),
            graph_spec(
                "rough",
                vec![displacement(
                    EffectInput::SourceGraphic,
                    EffectInput::SourceGraphic,
                    5.0,
                )],
            ),
        );

        let graphs = || {
            DiagramEffectSet::default()
                .with_graph(blur_graph("a", 1.0))
                .unwrap()
                .with_graph(blur_graph("b", 2.0))
                .unwrap()
        };
        assert_recipe_field_changes(
            "effects.binding.target",
            DiagramThemeSpec::new().with_effects(
                graphs()
                    .with_binding(EffectBinding::new(ThemeTarget::Node, "a").unwrap())
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_effects(
                graphs()
                    .with_binding(EffectBinding::new(ThemeTarget::Edge, "a").unwrap())
                    .unwrap(),
            ),
        );
        assert_recipe_field_changes(
            "effects.binding.effect_id",
            DiagramThemeSpec::new().with_effects(
                graphs()
                    .with_binding(EffectBinding::new(ThemeTarget::Node, "a").unwrap())
                    .unwrap(),
            ),
            DiagramThemeSpec::new().with_effects(
                graphs()
                    .with_binding(EffectBinding::new(ThemeTarget::Node, "b").unwrap())
                    .unwrap(),
            ),
        );
    }

    #[test]
    fn effect_binding_order_is_canonical_and_targets_are_unique() {
        let graphs = || {
            DiagramEffectSet::default()
                .with_graph(blur_graph("a", 1.0))
                .unwrap()
                .with_graph(blur_graph("b", 2.0))
                .unwrap()
        };
        let left = graphs()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "a").unwrap())
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::Edge, "b").unwrap())
            .unwrap();
        let right = graphs()
            .with_binding(EffectBinding::new(ThemeTarget::Edge, "b").unwrap())
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "a").unwrap())
            .unwrap();
        assert_eq!(
            compile(DiagramThemeSpec::new().with_effects(left)),
            compile(DiagramThemeSpec::new().with_effects(right))
        );

        let duplicate = graphs()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "a").unwrap())
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "b").unwrap());
        assert!(matches!(
            duplicate,
            Err(ThemeCompileValidationError::DuplicateId {
                field: "effects.binding.target"
            })
        ));
    }

    #[test]
    fn resource_and_requirement_domains_change_only_their_owned_identity() {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        let compile_asset =
            |id: &str, bytes: &[u8]| {
                DiagramThemeCompiler::new()
                    .compile(DiagramThemeSpec::new().with_assets(
                        ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                            FontAssetSpec::new(id, bytes),
                        ])),
                    ))
                    .expect("valid font catalog fixture")
            };

        let latin_a = compile_asset("fixture-a", latin);
        let latin_b = compile_asset("fixture-b", latin);
        let cjk_a = compile_asset("fixture-a", cjk);

        assert_eq!(
            latin_a.font_catalog().assets()[0].fingerprint(),
            latin_b.font_catalog().assets()[0].fingerprint(),
            "asset ids are catalog metadata, not font-byte identity"
        );
        assert_ne!(
            latin_a.report().font_catalog_fingerprint(),
            latin_b.report().font_catalog_fingerprint(),
            "catalog metadata must affect catalog identity"
        );
        assert_ne!(
            latin_a.recipe_fingerprint(),
            latin_b.recipe_fingerprint(),
            "catalog identity must affect recipe identity"
        );

        assert_ne!(
            latin_a.font_catalog().assets()[0].fingerprint(),
            cjk_a.font_catalog().assets()[0].fingerprint(),
            "font bytes must affect asset identity"
        );
        assert_ne!(
            latin_a.report().font_catalog_fingerprint(),
            cjk_a.report().font_catalog_fingerprint(),
            "asset identity must affect catalog identity"
        );
        assert_ne!(
            latin_a.recipe_fingerprint(),
            cjk_a.recipe_fingerprint(),
            "catalog resources retained by a recipe must affect recipe identity"
        );

        let left = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_requirements(
                    ThemeRequirements::new()
                        .with_required_capabilities([ThemeCapability::LetterSpacing]),
                ),
            )
            .unwrap();
        let right = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_requirements(
                ThemeRequirements::new().with_required_capabilities([ThemeCapability::WordSpacing]),
            ))
            .unwrap();
        assert_eq!(
            left.report().font_catalog_fingerprint(),
            right.report().font_catalog_fingerprint(),
            "requirements must not alter the retained catalog"
        );
        assert_ne!(
            left.recipe_fingerprint(),
            right.recipe_fingerprint(),
            "requirements are part of recipe intent"
        );
    }

    #[test]
    fn every_top_level_recipe_domain_changes_identity() {
        let baseline = compile(DiagramThemeSpec::new());
        let mermaid = DiagramThemeSpec::new().with_mermaid_compatibility(
            MermaidThemeCompatibility::default()
                .with_dark_mode(true)
                .unwrap()
                .with_variable("primaryColor", "#112233")
                .unwrap(),
        );
        let typography = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_default(
                TextStyle::default()
                    .with_font_size_px(18.0)
                    .expect("valid font size"),
            ),
        );
        let styles =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::Solid(color("#112233"))),
            )));
        let stops = [
            GradientStop::new(0.0, color("#000000")).unwrap(),
            GradientStop::new(1.0, color("#ffffff")).unwrap(),
        ];
        let canvas = DiagramThemeSpec::new().with_canvas(CanvasSpec::default().with_base(
            CanvasPaint::LinearGradient(super::super::LinearGradient::new(30.0, stops).unwrap()),
        ));
        let effects = DiagramThemeSpec::new().with_effects(
            DiagramEffectSet::default()
                .with_graph(blur_graph("soft", 1.5))
                .unwrap(),
        );
        let requirements = DiagramThemeSpec::new().with_requirements(
            ThemeRequirements::new().with_required_capabilities([ThemeCapability::Opacity]),
        );
        let font_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let assets = DiagramThemeSpec::new().with_assets(ThemeAssets::default().with_font_catalog(
            FontCatalogSpec::new([FontAssetSpec::new("excalifont", font_bytes)]),
        ));

        for (name, spec) in [
            ("mermaid", mermaid),
            ("typography", typography),
            ("styles", styles),
            ("canvas", canvas),
            ("effects", effects),
            ("requirements", requirements),
            ("assets", assets),
        ] {
            assert_ne!(
                baseline,
                compile(spec),
                "{name} must affect recipe identity"
            );
        }
    }
}
