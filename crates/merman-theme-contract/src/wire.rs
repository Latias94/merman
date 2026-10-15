use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::finite::ContainsNonFiniteNumber;

/// A style entry in the flat version 1 rule-set wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
#[expect(
    clippy::large_enum_variant,
    reason = "Rules dominate the bounded style list; keep their patches inline without an allocation per rule"
)]
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
        /// The style patch.
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

/// Omission, explicit clear, or a value for a clearable style facet.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum SpecifiedWireV1<T> {
    /// The facet was omitted.
    #[default]
    Unspecified,
    /// The facet was explicitly cleared with JSON `null`.
    Clear,
    /// The facet carries a value.
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
    /// Optional stroke patch; explicit JSON `null` is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub stroke: Option<ThemeStrokePatchWireV1>,
    /// Geometry radius in pixels, interpreted by family and target (for example, Flowchart
    /// Node corners or Quadrant ChartSeries points). A family may explicitly classify a
    /// numeric request as inapplicable to a shape without that geometry channel; this does
    /// not certify an unimplemented writer or an unsupported operation such as Clear.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub radius: SpecifiedWireV1<f32>,
    /// Padding in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub padding: SpecifiedWireV1<ThemeInsetsWireV1>,
    /// Optional typography patch; explicit JSON `null` is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub typography: Option<ThemeTextStylePatchWireV1>,
    /// Effect graph identifier.
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

/// A paint value accepted by the version 1 wires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeCanvasPaintWireV1 {
    /// A compact color string, including `"transparent"`.
    Color(String),
    /// A tagged structured paint.
    Structured(ThemeCanvasPaintObjectWireV1),
}

/// A tagged structured paint accepted by the version 1 wires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeCanvasPaintObjectWireV1 {
    /// Fully transparent paint.
    Transparent,
    /// Solid color paint.
    Solid {
        /// The raw color value.
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
    /// Raw stop offset; the renderer-owned decoder validates the supported range.
    pub offset: f32,
    /// Raw stop color.
    pub color: String,
}

/// A theme length in the number, pixel-object, or percent-object wire shape.
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

/// Insets in the compact or four-sided wire shape.
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

/// Line height in the keyword, multiplier, or pixel-object wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ThemeLineHeightWireV1 {
    /// A keyword such as `"normal"`; the renderer decoder validates the domain.
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

pub(crate) fn deserialize_optional_non_null<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl ContainsNonFiniteNumber for ThemeRuleSetWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Rule {
                target: _,
                family: _,
                variant: _,
                ordinal: _,
                style,
            } => style.contains_non_finite_number(),
            Self::OrdinalPalette {
                target: _,
                colors: _,
            } => false,
        }
    }
}

impl ContainsNonFiniteNumber for ThemeStylePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            fill,
            opacity,
            fill_opacity,
            stroke,
            radius,
            padding,
            typography,
            effect: _,
        } = self;

        fill.contains_non_finite_number()
            || opacity.contains_non_finite_number()
            || fill_opacity.contains_non_finite_number()
            || stroke.contains_non_finite_number()
            || radius.contains_non_finite_number()
            || padding.contains_non_finite_number()
            || typography.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeStrokePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            paint,
            width,
            dasharray,
            linecap: _,
            linejoin: _,
            opacity,
        } = self;

        paint.contains_non_finite_number()
            || width.contains_non_finite_number()
            || dasharray.contains_non_finite_number()
            || opacity.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeTextStylePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            font_stack: _,
            font_size_px,
            font_weight: _,
            font_style: _,
            line_height,
            letter_spacing_px,
            word_spacing_px,
            transform: _,
            decoration: _,
            text_align: _,
            white_space: _,
            wrap: _,
        } = self;

        font_size_px.contains_non_finite_number()
            || line_height.contains_non_finite_number()
            || letter_spacing_px.contains_non_finite_number()
            || word_spacing_px.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeCanvasPaintWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Color(_) => false,
            Self::Structured(paint) => paint.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeCanvasPaintObjectWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Transparent => false,
            Self::Solid { color: _ } => false,
            Self::LinearGradient {
                angle_degrees,
                stops,
                repetition,
            } => {
                angle_degrees.contains_non_finite_number()
                    || stops.contains_non_finite_number()
                    || repetition.contains_non_finite_number()
            }
            Self::RadialGradient {
                center_x,
                center_y,
                radius,
                stops,
                repetition,
            } => {
                center_x.contains_non_finite_number()
                    || center_y.contains_non_finite_number()
                    || radius.contains_non_finite_number()
                    || stops.contains_non_finite_number()
                    || repetition.contains_non_finite_number()
            }
            Self::Pattern {
                pattern: _,
                cell_width,
                cell_height,
                foreground: _,
                background: _,
                angle_degrees,
            } => {
                cell_width.contains_non_finite_number()
                    || cell_height.contains_non_finite_number()
                    || angle_degrees.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeLinearGradientRepetitionWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Repeating { period_px } => period_px.contains_non_finite_number(),
            Self::Tiled {
                width_px,
                height_px,
            } => width_px.contains_non_finite_number() || height_px.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeRadialGradientRepetitionWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Repeating => false,
            Self::Tiled {
                width_px,
                height_px,
            } => width_px.contains_non_finite_number() || height_px.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeGradientStopWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self { offset, color: _ } = self;

        offset.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeLengthWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::PxValue(value) | Self::Px { px: value } | Self::Percent { percent: value } => {
                value.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeInsetsWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::All(value) => value.contains_non_finite_number(),
            Self::Sides {
                top,
                right,
                bottom,
                left,
            } => {
                top.contains_non_finite_number()
                    || right.contains_non_finite_number()
                    || bottom.contains_non_finite_number()
                    || left.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeLineHeightWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Keyword(_) => false,
            Self::Multiplier(value) | Self::Px { px: value } => value.contains_non_finite_number(),
        }
    }
}
