use merman_core::DiagramFamilyId;
use merman_render::diagram_theme::{ThemeTarget, ThemeVariant};
use merman_theme_contract::{
    SpecifiedWireV1, ThemeCanvasPaintWireV1, ThemeDefinitionV1, ThemeOrdinalCycleWireV1,
    ThemeOrdinalSelectorWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1,
    ThemeTextStylePatchWireV1, ThemeTokensV1,
};

/// Builds one shareable version 1 theme definition without introducing another wire shape.
///
/// Rule and palette calls append entries in source order. Validation remains owned by the shared
/// materializer and compiler.
#[derive(Debug, Clone)]
pub struct ThemeDefinitionBuilderV1 {
    tokens: ThemeTokensV1,
    styles: Vec<ThemeRuleSetWireV1>,
}

impl ThemeDefinitionBuilderV1 {
    /// Starts a definition with the authored token object.
    pub fn new(tokens: ThemeTokensV1) -> Self {
        Self {
            tokens,
            styles: Vec::new(),
        }
    }

    /// Appends one typed semantic rule.
    pub fn with_rule(mut self, rule: ThemeRuleBuilderV1) -> Self {
        self.styles.push(rule.into_wire());
        self
    }

    /// Appends one ordinal palette using the authoritative semantic target identifier.
    pub fn with_ordinal_palette<I, C>(mut self, target: ThemeTarget, colors: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<String>,
    {
        self.styles.push(ThemeRuleSetWireV1::OrdinalPalette {
            target: target.id().to_owned(),
            colors: colors.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Produces the existing contract-owned definition value.
    pub fn build(self) -> ThemeDefinitionV1 {
        ThemeDefinitionV1::new(self.tokens).with_styles(self.styles)
    }
}

/// Builds one version 1 authored rule while keeping semantic identifiers typed.
///
/// An untouched facet is `Unspecified`, [`Self::clear`] writes an atomic `Clear`, and each
/// `with_*` method writes a `Value`. The builder deliberately performs no semantic validation.
#[derive(Debug, Clone)]
pub struct ThemeRuleBuilderV1 {
    target: ThemeTarget,
    family: Option<DiagramFamilyId>,
    variant: Option<ThemeVariant>,
    ordinal: Option<ThemeOrdinalSelectorWireV1>,
    style: ThemeStylePatchWireV1,
}

impl ThemeRuleBuilderV1 {
    /// Starts a rule for one semantic target without a variant qualifier.
    pub fn new(target: ThemeTarget) -> Self {
        Self {
            target,
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1::default(),
        }
    }

    /// Restricts the rule to one diagram family.
    pub fn for_family(mut self, family: DiagramFamilyId) -> Self {
        self.family = Some(family);
        self
    }

    /// Restricts the rule to one semantic variant.
    pub fn with_variant(mut self, variant: ThemeVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Restricts the rule to one exact one-based ordinal.
    pub fn with_exact_ordinal(mut self, exact: u32) -> Self {
        self.ordinal = Some(ThemeOrdinalSelectorWireV1::Exact { exact });
        self
    }

    /// Restricts the rule to a cycle with a zero-based offset.
    pub fn with_ordinal_cycle(mut self, period: u32, offset: u32) -> Self {
        self.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 { period, offset },
        });
        self
    }

    /// Replaces the complete style patch with the contract-owned wire value.
    ///
    /// This is the advanced escape hatch for facets without a named convenience below. It does
    /// not parse, validate, or reinterpret the patch.
    pub fn with_style_patch(mut self, style: ThemeStylePatchWireV1) -> Self {
        self.style = style;
        self
    }

    /// Sets the fill to a compact color value.
    pub fn with_fill_color(mut self, color: impl Into<String>) -> Self {
        self.style.fill = SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(color.into()));
        self
    }

    /// Sets the stroke to a compact color value.
    pub fn with_stroke_color(mut self, color: impl Into<String>) -> Self {
        self.stroke_mut().paint =
            SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(color.into()));
        self
    }

    /// Sets the stroke width in pixels.
    pub fn with_stroke_width(mut self, width: f32) -> Self {
        self.stroke_mut().width = SpecifiedWireV1::Value(width);
        self
    }

    /// Sets the corner radius in pixels.
    pub fn with_radius(mut self, radius: f32) -> Self {
        self.style.radius = SpecifiedWireV1::Value(radius);
        self
    }

    /// Explicitly clears one atomic style facet.
    ///
    /// Stroke and typography groups remain ordinary objects; this method never creates a group-
    /// level `null` value.
    pub fn clear(mut self, facet: ThemeRuleFacetV1) -> Self {
        match facet {
            ThemeRuleFacetV1::Fill => self.style.fill = SpecifiedWireV1::Clear,
            ThemeRuleFacetV1::Opacity => self.style.opacity = SpecifiedWireV1::Clear,
            ThemeRuleFacetV1::FillOpacity => self.style.fill_opacity = SpecifiedWireV1::Clear,
            ThemeRuleFacetV1::StrokePaint => {
                self.stroke_mut().paint = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::StrokeWidth => {
                self.stroke_mut().width = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::StrokeDasharray => {
                self.stroke_mut().dasharray = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::StrokeLineCap => {
                self.stroke_mut().linecap = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::StrokeLineJoin => {
                self.stroke_mut().linejoin = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::StrokeOpacity => {
                self.stroke_mut().opacity = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::Radius => self.style.radius = SpecifiedWireV1::Clear,
            ThemeRuleFacetV1::Padding => self.style.padding = SpecifiedWireV1::Clear,
            ThemeRuleFacetV1::FontStack => {
                self.typography_mut().font_stack = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::FontSize => {
                self.typography_mut().font_size_px = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::FontWeight => {
                self.typography_mut().font_weight = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::FontStyle => {
                self.typography_mut().font_style = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::LineHeight => {
                self.typography_mut().line_height = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::LetterSpacing => {
                self.typography_mut().letter_spacing_px = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::WordSpacing => {
                self.typography_mut().word_spacing_px = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::TextTransform => {
                self.typography_mut().transform = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::TextDecoration => {
                self.typography_mut().decoration = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::TextAlign => {
                self.typography_mut().text_align = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::WhiteSpace => {
                self.typography_mut().white_space = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::Wrap => {
                self.typography_mut().wrap = SpecifiedWireV1::Clear;
            }
            ThemeRuleFacetV1::Effect => self.style.effect = SpecifiedWireV1::Clear,
        }
        self
    }

    fn stroke_mut(&mut self) -> &mut ThemeStrokePatchWireV1 {
        self.style
            .stroke
            .get_or_insert_with(ThemeStrokePatchWireV1::default)
    }

    fn typography_mut(&mut self) -> &mut ThemeTextStylePatchWireV1 {
        self.style
            .typography
            .get_or_insert_with(ThemeTextStylePatchWireV1::default)
    }

    fn into_wire(self) -> ThemeRuleSetWireV1 {
        ThemeRuleSetWireV1::Rule {
            target: self.target.id().to_owned(),
            family: self.family.map(|family| family.as_str().to_owned()),
            variant: self.variant.map(|variant| variant.id().to_owned()),
            ordinal: self.ordinal,
            style: self.style,
        }
    }
}

/// An atomic version 1 style facet that can be explicitly cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeRuleFacetV1 {
    /// Fill paint.
    Fill,
    /// Overall opacity.
    Opacity,
    /// Fill-specific opacity.
    FillOpacity,
    /// Stroke paint.
    StrokePaint,
    /// Stroke width.
    StrokeWidth,
    /// Stroke dash lengths.
    StrokeDasharray,
    /// Stroke line cap.
    StrokeLineCap,
    /// Stroke line join.
    StrokeLineJoin,
    /// Stroke-specific opacity.
    StrokeOpacity,
    /// Corner radius.
    Radius,
    /// Content padding.
    Padding,
    /// Font-family stack.
    FontStack,
    /// Font size.
    FontSize,
    /// Font weight.
    FontWeight,
    /// Font style.
    FontStyle,
    /// Line height.
    LineHeight,
    /// Letter spacing.
    LetterSpacing,
    /// Word spacing.
    WordSpacing,
    /// Text transform.
    TextTransform,
    /// Text decoration.
    TextDecoration,
    /// Text alignment.
    TextAlign,
    /// White-space behavior.
    WhiteSpace,
    /// Text wrapping mode.
    Wrap,
    /// Effect graph reference.
    Effect,
}
