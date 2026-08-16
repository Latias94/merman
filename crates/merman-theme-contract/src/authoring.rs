use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonical_json::{CanonicalJsonError, canonical_json_bytes};
use crate::version::THEME_CONTRACT_V1;

mod finite;

use finite::ContainsNonFiniteNumber;

/// The closed version 1 theme-authoring envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDefinitionV1 {
    tokens: ThemeTokensV1,
    styles: Vec<ThemeRuleSetWireV1>,
}

impl ThemeDefinitionV1 {
    /// Creates an ephemeral definition using the only supported version 1 tuple.
    pub fn new(tokens: ThemeTokensV1) -> Self {
        Self {
            tokens,
            styles: Vec::new(),
        }
    }

    /// Replaces the authored style entries while preserving their source order.
    pub fn with_styles(mut self, styles: Vec<ThemeRuleSetWireV1>) -> Self {
        self.styles = styles;
        self
    }

    /// Returns the persisted authoring schema version.
    pub const fn authoring_schema_version(&self) -> u32 {
        THEME_CONTRACT_V1.authoring_schema_version()
    }

    /// Returns the deterministic expansion-table version.
    pub const fn expansion_version(&self) -> u32 {
        THEME_CONTRACT_V1.expansion_version()
    }

    /// Returns the authored tokens without applying defaults.
    pub const fn tokens(&self) -> &ThemeTokensV1 {
        &self.tokens
    }

    /// Returns the authored style entries in source order.
    pub fn styles(&self) -> &[ThemeRuleSetWireV1] {
        &self.styles
    }

    /// Serializes this typed definition using RFC 8785 JSON Canonicalization Scheme.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        if self.contains_non_finite_number() {
            return Err(CanonicalJsonError::invalid_input(
                "theme definition contains a non-finite number",
            ));
        }
        canonical_json_bytes(&self.encode())
    }

    fn contains_non_finite_number(&self) -> bool {
        self.tokens.contains_non_finite_number()
            || self
                .styles
                .iter()
                .any(ContainsNonFiniteNumber::contains_non_finite_number)
    }

    const fn encode(&self) -> ThemeDefinitionV1Encode<'_> {
        ThemeDefinitionV1Encode {
            authoring_schema_version: THEME_CONTRACT_V1.authoring_schema_version(),
            expansion_version: THEME_CONTRACT_V1.expansion_version(),
            tokens: &self.tokens,
            styles: &self.styles,
        }
    }
}

#[derive(Serialize)]
struct ThemeDefinitionV1Encode<'a> {
    authoring_schema_version: u32,
    expansion_version: u32,
    tokens: &'a ThemeTokensV1,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    styles: &'a Vec<ThemeRuleSetWireV1>,
}

impl Serialize for ThemeDefinitionV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.contains_non_finite_number() {
            return Err(S::Error::custom(
                "theme definition contains a non-finite number",
            ));
        }
        self.encode().serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeDefinitionV1Decode {
    authoring_schema_version: u32,
    expansion_version: u32,
    tokens: ThemeTokensV1,
    #[serde(default)]
    styles: Vec<ThemeRuleSetWireV1>,
}

impl<'de> Deserialize<'de> for ThemeDefinitionV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let decoded = ThemeDefinitionV1Decode::deserialize(deserializer)?;
        if decoded.authoring_schema_version != THEME_CONTRACT_V1.authoring_schema_version()
            || decoded.expansion_version != THEME_CONTRACT_V1.expansion_version()
        {
            return Err(D::Error::custom(format_args!(
                "ThemeDefinitionV1 requires authoring version tuple ({}, {})",
                THEME_CONTRACT_V1.authoring_schema_version(),
                THEME_CONTRACT_V1.expansion_version()
            )));
        }
        Ok(Self {
            tokens: decoded.tokens,
            styles: decoded.styles,
        })
    }
}

/// A cross-family color token in [`ThemeTokensV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeColorTokenV1 {
    /// The diagram canvas color.
    Canvas,
    /// The primary surface color.
    Surface,
    /// The alternate surface color.
    SurfaceAlt,
    /// The muted surface color.
    SurfaceMuted,
    /// The primary text color.
    Text,
    /// The secondary text color.
    SubtleText,
    /// The border color.
    Border,
    /// The line color.
    Line,
    /// The accent color.
    Accent,
    /// The error color.
    Error,
    /// The warning color.
    Warning,
    /// The success color.
    Success,
}

impl ThemeColorTokenV1 {
    /// Every version 1 color token in contract order.
    pub const ALL: [Self; 12] = [
        Self::Canvas,
        Self::Surface,
        Self::SurfaceAlt,
        Self::SurfaceMuted,
        Self::Text,
        Self::SubtleText,
        Self::Border,
        Self::Line,
        Self::Accent,
        Self::Error,
        Self::Warning,
        Self::Success,
    ];
}

/// The closed version 1 cross-family token object.
///
/// Every field is optional. Omission is retained until materialization applies the versioned
/// defaults; this wire type deliberately performs no token-value validation.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTokensV1 {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    canvas: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    surface: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    surface_alt: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    surface_muted: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    text: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    subtle_text: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    border: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    line: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    accent: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    error: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    warning: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    success: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    series: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    typography: Option<ThemeAuthoringTypographyV1>,
}

impl ThemeTokensV1 {
    /// Sets one cross-family color token without validating its materialized color domain.
    pub fn with_color(mut self, token: ThemeColorTokenV1, value: impl Into<String>) -> Self {
        let slot = match token {
            ThemeColorTokenV1::Canvas => &mut self.canvas,
            ThemeColorTokenV1::Surface => &mut self.surface,
            ThemeColorTokenV1::SurfaceAlt => &mut self.surface_alt,
            ThemeColorTokenV1::SurfaceMuted => &mut self.surface_muted,
            ThemeColorTokenV1::Text => &mut self.text,
            ThemeColorTokenV1::SubtleText => &mut self.subtle_text,
            ThemeColorTokenV1::Border => &mut self.border,
            ThemeColorTokenV1::Line => &mut self.line,
            ThemeColorTokenV1::Accent => &mut self.accent,
            ThemeColorTokenV1::Error => &mut self.error,
            ThemeColorTokenV1::Warning => &mut self.warning,
            ThemeColorTokenV1::Success => &mut self.success,
        };
        *slot = Some(value.into());
        self
    }

    /// Returns one authored color token, or `None` when it was omitted.
    pub fn color(&self, token: ThemeColorTokenV1) -> Option<&str> {
        match token {
            ThemeColorTokenV1::Canvas => self.canvas.as_deref(),
            ThemeColorTokenV1::Surface => self.surface.as_deref(),
            ThemeColorTokenV1::SurfaceAlt => self.surface_alt.as_deref(),
            ThemeColorTokenV1::SurfaceMuted => self.surface_muted.as_deref(),
            ThemeColorTokenV1::Text => self.text.as_deref(),
            ThemeColorTokenV1::SubtleText => self.subtle_text.as_deref(),
            ThemeColorTokenV1::Border => self.border.as_deref(),
            ThemeColorTokenV1::Line => self.line.as_deref(),
            ThemeColorTokenV1::Accent => self.accent.as_deref(),
            ThemeColorTokenV1::Error => self.error.as_deref(),
            ThemeColorTokenV1::Warning => self.warning.as_deref(),
            ThemeColorTokenV1::Success => self.success.as_deref(),
        }
    }

    /// Sets the authored series, including an explicitly empty series for later validation.
    pub fn with_series(mut self, series: Vec<String>) -> Self {
        self.series = Some(series);
        self
    }

    /// Returns the authored series, preserving omission separately from an empty list.
    pub fn series(&self) -> Option<&[String]> {
        self.series.as_deref()
    }

    /// Sets the authored typography object.
    pub fn with_typography(mut self, typography: ThemeAuthoringTypographyV1) -> Self {
        self.typography = Some(typography);
        self
    }

    /// Returns the authored typography object, or `None` when it was omitted.
    pub const fn typography(&self) -> Option<&ThemeAuthoringTypographyV1> {
        self.typography.as_ref()
    }

    fn contains_non_finite_number(&self) -> bool {
        self.typography.contains_non_finite_number()
    }
}

/// The closed version 1 authoring typography object.
///
/// Every field is optional and retains omission until materialization.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeAuthoringTypographyV1 {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    font_stack: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    font_size_px: Option<f32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    font_weight: Option<u16>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    line_height: Option<ThemeLineHeightWireV1>,
}

impl ThemeAuthoringTypographyV1 {
    /// Sets the authored font stack, including an empty list for later semantic validation.
    pub fn with_font_stack(mut self, font_stack: Vec<String>) -> Self {
        self.font_stack = Some(font_stack);
        self
    }

    /// Sets the authored font size without applying materializer validation.
    pub const fn with_font_size_px(mut self, font_size_px: f32) -> Self {
        self.font_size_px = Some(font_size_px);
        self
    }

    /// Sets the authored font weight without applying materializer validation.
    pub const fn with_font_weight(mut self, font_weight: u16) -> Self {
        self.font_weight = Some(font_weight);
        self
    }

    /// Sets the authored line height without applying materializer validation.
    pub fn with_line_height(mut self, line_height: ThemeLineHeightWireV1) -> Self {
        self.line_height = Some(line_height);
        self
    }

    /// Returns the authored font stack, preserving omission separately from an empty list.
    pub fn font_stack(&self) -> Option<&[String]> {
        self.font_stack.as_deref()
    }

    /// Returns the authored font size.
    pub const fn font_size_px(&self) -> Option<f32> {
        self.font_size_px
    }

    /// Returns the authored font weight.
    pub const fn font_weight(&self) -> Option<u16> {
        self.font_weight
    }

    /// Returns the authored line height.
    pub const fn line_height(&self) -> Option<&ThemeLineHeightWireV1> {
        self.line_height.as_ref()
    }
}

/// A style entry in the flat version 1 authored rule-set wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeRuleSetWireV1 {
    /// A selector and style patch.
    Rule {
        /// The semantic target identifier.
        target: String,
        /// An optional diagram-family identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        family: Option<String>,
        /// An optional semantic variant identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        variant: Option<String>,
        /// An optional ordinal selector.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        ordinal: Option<ThemeOrdinalSelectorWireV1>,
        /// The authored style patch.
        style: ThemeStylePatchWireV1,
    },
    /// A palette applied by ordinal to one semantic target.
    OrdinalPalette {
        /// The semantic target identifier.
        target: String,
        /// The palette colors in ordinal order.
        colors: Vec<String>,
    },
}

/// Omission, explicit clear, or an authored value for a clearable style facet.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum SpecifiedWireV1<T> {
    /// The facet was omitted.
    #[default]
    Unspecified,
    /// The facet was explicitly cleared with JSON `null`.
    Clear,
    /// The facet carries an authored value.
    Value(T),
}

impl<T> SpecifiedWireV1<T> {
    /// Returns whether this facet should be omitted during serialization.
    pub const fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }
}

impl<T> Serialize for SpecifiedWireV1<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Unspecified => Err(S::Error::custom(
                "an unspecified style facet must be omitted by its containing wire object",
            )),
            Self::Clear => serializer.serialize_none(),
            Self::Value(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T> Deserialize<'de> for SpecifiedWireV1<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Clear,
        })
    }
}

/// The closed version 1 style-patch wire.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeStylePatchWireV1 {
    /// Fill paint.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub fill: SpecifiedWireV1<ThemeCanvasPaintWireV1>,
    /// Overall paint opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub opacity: SpecifiedWireV1<f32>,
    /// Fill-specific opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub fill_opacity: SpecifiedWireV1<f32>,
    /// Stroke patch.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub stroke: SpecifiedWireV1<ThemeStrokePatchWireV1>,
    /// Corner radius in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub radius: SpecifiedWireV1<f32>,
    /// Padding in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub padding: SpecifiedWireV1<ThemeInsetsWireV1>,
    /// Typography patch.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub typography: SpecifiedWireV1<ThemeTextStylePatchWireV1>,
    /// Effect graph identifier; semantic rejection belongs to materialization.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub effect: SpecifiedWireV1<String>,
}

/// The closed version 1 stroke-patch wire.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeStrokePatchWireV1 {
    /// Stroke paint.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub paint: SpecifiedWireV1<ThemeCanvasPaintWireV1>,
    /// Stroke width in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub width: SpecifiedWireV1<f32>,
    /// Stroke dash lengths.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub dasharray: SpecifiedWireV1<Vec<f32>>,
    /// Stroke line-cap identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub linecap: SpecifiedWireV1<String>,
    /// Stroke line-join identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub linejoin: SpecifiedWireV1<String>,
    /// Stroke-specific opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub opacity: SpecifiedWireV1<f32>,
}

/// The closed version 1 text-style-patch wire.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTextStylePatchWireV1 {
    /// Font-family stack.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_stack: SpecifiedWireV1<Vec<String>>,
    /// Font size in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_size_px: SpecifiedWireV1<f32>,
    /// Numeric font weight.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_weight: SpecifiedWireV1<u16>,
    /// Font-style identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_style: SpecifiedWireV1<String>,
    /// Line-height value.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub line_height: SpecifiedWireV1<ThemeLineHeightWireV1>,
    /// Letter spacing in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub letter_spacing_px: SpecifiedWireV1<f32>,
    /// Word spacing in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub word_spacing_px: SpecifiedWireV1<f32>,
    /// Text-transform identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub transform: SpecifiedWireV1<String>,
    /// Text-decoration identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub decoration: SpecifiedWireV1<String>,
    /// Text-alignment identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub text_align: SpecifiedWireV1<String>,
    /// White-space identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub white_space: SpecifiedWireV1<String>,
    /// Text-wrap identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub wrap: SpecifiedWireV1<String>,
}

/// A paint value accepted by the version 1 style wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeCanvasPaintWireV1 {
    /// A compact color string, including `"transparent"`.
    Color(String),
    /// A tagged structured paint.
    Structured(ThemeCanvasPaintObjectWireV1),
}

/// A tagged structured paint accepted by the version 1 style wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeCanvasPaintObjectWireV1 {
    /// Fully transparent paint.
    Transparent,
    /// Solid color paint.
    Solid {
        /// The color value.
        color: String,
    },
    /// Linear-gradient paint.
    LinearGradient {
        /// Gradient angle in degrees.
        angle_degrees: f32,
        /// Gradient stops in source order.
        stops: Vec<ThemeGradientStopWireV1>,
        /// Optional repetition behavior.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        repetition: Option<ThemeLinearGradientRepetitionWireV1>,
    },
    /// Radial-gradient paint.
    RadialGradient {
        /// Horizontal center.
        center_x: ThemeLengthWireV1,
        /// Vertical center.
        center_y: ThemeLengthWireV1,
        /// Gradient radius.
        radius: ThemeLengthWireV1,
        /// Gradient stops in source order.
        stops: Vec<ThemeGradientStopWireV1>,
        /// Optional repetition behavior.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        repetition: Option<ThemeRadialGradientRepetitionWireV1>,
    },
    /// Pattern paint.
    Pattern {
        /// Pattern-kind identifier.
        pattern: String,
        /// Pattern cell width.
        cell_width: f32,
        /// Pattern cell height.
        cell_height: f32,
        /// Foreground color.
        foreground: String,
        /// Optional background color.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        background: Option<String>,
        /// Optional pattern angle in degrees.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        angle_degrees: Option<f32>,
    },
}

/// Linear-gradient repetition behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeLinearGradientRepetitionWireV1 {
    /// Repeats the gradient after one pixel period.
    Repeating {
        /// Repetition period in pixels.
        period_px: f32,
    },
    /// Tiles the gradient in a fixed pixel rectangle.
    Tiled {
        /// Tile width in pixels.
        width_px: f32,
        /// Tile height in pixels.
        height_px: f32,
    },
}

/// Radial-gradient repetition behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeRadialGradientRepetitionWireV1 {
    /// Repeats the radial gradient.
    Repeating,
    /// Tiles the gradient in a fixed pixel rectangle.
    Tiled {
        /// Tile width in pixels.
        width_px: f32,
        /// Tile height in pixels.
        height_px: f32,
    },
}

/// One color stop in a gradient paint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeGradientStopWireV1 {
    /// Normalized stop offset.
    pub offset: f32,
    /// Stop color.
    pub color: String,
}

/// A theme length in the existing number, pixel-object, or percent-object wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeLengthWireV1 {
    /// A compact pixel value.
    PxValue(f32),
    /// An explicit pixel value.
    Px {
        /// Pixel length.
        px: f32,
    },
    /// A percentage value.
    Percent {
        /// Percentage length.
        percent: f32,
    },
}

/// Insets in the existing compact or four-sided wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeInsetsWireV1 {
    /// Applies one pixel inset to every side.
    All(f32),
    /// Carries an explicit inset for every side.
    Sides {
        /// Top inset in pixels.
        top: f32,
        /// Right inset in pixels.
        right: f32,
        /// Bottom inset in pixels.
        bottom: f32,
        /// Left inset in pixels.
        left: f32,
    },
}

/// Line height in the existing keyword, multiplier, or pixel-object wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeLineHeightWireV1 {
    /// A keyword such as `"normal"`; materialization validates the domain.
    Keyword(String),
    /// A unitless multiplier.
    Multiplier(f32),
    /// An explicit pixel line height.
    Px {
        /// Pixel line height.
        px: f32,
    },
}

/// An exact or cyclic ordinal selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeOrdinalSelectorWireV1 {
    /// Matches one exact one-based ordinal.
    Exact {
        /// Exact ordinal.
        exact: u32,
    },
    /// Matches ordinals using a cycle.
    Cycle {
        /// Cycle parameters.
        cycle: ThemeOrdinalCycleWireV1,
    },
}

/// Parameters for a cyclic ordinal selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeOrdinalCycleWireV1 {
    /// Cycle period.
    pub period: u32,
    /// Offset inside the period.
    pub offset: u32,
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
