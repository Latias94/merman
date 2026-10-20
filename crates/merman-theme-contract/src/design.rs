use std::collections::BTreeMap;

use serde::ser::{Error as _, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonical_json::{CanonicalJsonError, canonical_json_bytes};
use crate::finite::ContainsNonFiniteNumber;
use crate::spec::{
    MermaidThemeCompatibilityWireV1, ThemeAssetsWireV1, ThemeEffectPrimitiveWireV1,
    ThemeRequirementsWireV1,
};
use crate::wire::{
    SpecifiedWireV1, ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1,
    ThemeLinearGradientRepetitionWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRadialGradientRepetitionWireV1, deserialize_optional_non_null,
};

// Derived Serde structs also accept positional sequences. Authoring objects are JSON maps;
// delegate their field validation to the derive while excluding that alternate representation.
macro_rules! object_deserialize {
    ($($ty:ty),+ $(,)?) => {$ (
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct ObjectVisitor;
                impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
                    type Value = $ty;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        formatter.write_str("a theme object")
                    }
                    fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                        <$ty>::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    }
                }
                deserializer.deserialize_map(ObjectVisitor)
            }
        }
    )+};
}

macro_rules! object_serialize {
    ($($ty:ty),+ $(,)?) => {$ (
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                <$ty>::serialize(self, serializer)
            }
        }
    )+};
}

/// A direct typed value or an exact, document-local token reference.
///
/// References never interpret dots as traversal, interpolate strings, or convert token types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeValueV1<T> {
    /// A reference to a named value of the consumer's required type.
    Ref(ThemeReferenceV1),
    /// A direct value with no shared binding.
    Literal(T),
}

/// A closed document-local reference object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeReferenceV1 {
    /// The exact token identifier, including any literal dots.
    pub r#ref: String,
}

impl<'de> Deserialize<'de> for ThemeReferenceV1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde also admits sequences for derived structs. In an untagged value that would
        // misread a one-element palette/font stack as a reference instead of a literal list.
        struct ReferenceVisitor;
        impl<'de> serde::de::Visitor<'de> for ReferenceVisitor {
            type Value = ThemeReferenceV1;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object containing exactly one ref string")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct ReferenceObject {
                    r#ref: String,
                }

                let value = ReferenceObject::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )?;
                Ok(ThemeReferenceV1 { r#ref: value.r#ref })
            }
        }
        deserializer.deserialize_map(ReferenceVisitor)
    }
}

/// A general editable theme design with reusable typed values and semantic styles.
///
/// Omitted collections are empty. Explicit JSON `null` is rejected except at clearable style
/// fields. Reference resolution and consumer-domain validation belong to the Rust materializer.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignV1 {
    /// Named reusable values, keyed by stable document-local identifiers.
    #[serde(default)]
    pub tokens: BTreeMap<String, ThemeDesignTokenV1>,
    /// Semantic defaults, applied in deterministic target-identifier order.
    #[serde(default)]
    pub styles: BTreeMap<String, ThemeDesignStyleV1>,
    /// Scoped exceptions, applied after defaults in their authored order.
    #[serde(default)]
    pub rules: Vec<ThemeDesignRuleV1>,
    /// Ordinal color palettes keyed by semantic target identifier.
    #[serde(default)]
    pub palettes: BTreeMap<String, ThemeValueV1<Vec<ThemeValueV1<String>>>>,
    /// Optional authored canvas.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub canvas: Option<ThemeDesignCanvasV1>,
    /// Optional default and family typography.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub typography: Option<ThemeDesignTypographyV1>,
    /// Optional literal Mermaid compatibility settings.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub mermaid: Option<MermaidThemeCompatibilityWireV1>,
    /// Optional embedded assets and font policy.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub assets: Option<ThemeAssetsWireV1>,
    /// Optional declared capability requirements.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub requirements: Option<ThemeRequirementsWireV1>,
}

impl ThemeDesignV1 {
    /// Serializes the editable design using RFC 8785 JSON Canonicalization Scheme.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        if self.contains_non_finite_number() {
            return Err(CanonicalJsonError::invalid_input(
                "theme design contains a non-finite number",
            ));
        }
        canonical_json_bytes(self)
    }
}

impl Serialize for ThemeDesignV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.contains_non_finite_number() {
            return Err(S::Error::custom(
                "theme design contains a non-finite number",
            ));
        }
        let field_count = usize::from(!self.tokens.is_empty())
            + usize::from(!self.styles.is_empty())
            + usize::from(!self.rules.is_empty())
            + usize::from(!self.palettes.is_empty())
            + usize::from(self.canvas.is_some())
            + usize::from(self.typography.is_some())
            + usize::from(self.mermaid.is_some())
            + usize::from(self.assets.is_some())
            + usize::from(self.requirements.is_some());
        let mut state = serializer.serialize_struct("ThemeDesignV1", field_count)?;
        if !self.tokens.is_empty() {
            state.serialize_field("tokens", &self.tokens)?;
        }
        if !self.styles.is_empty() {
            state.serialize_field("styles", &self.styles)?;
        }
        if !self.rules.is_empty() {
            state.serialize_field("rules", &self.rules)?;
        }
        if !self.palettes.is_empty() {
            state.serialize_field("palettes", &self.palettes)?;
        }
        if let Some(canvas) = &self.canvas {
            state.serialize_field("canvas", canvas)?;
        }
        if let Some(typography) = &self.typography {
            state.serialize_field("typography", typography)?;
        }
        if let Some(mermaid) = &self.mermaid {
            state.serialize_field("mermaid", mermaid)?;
        }
        if let Some(assets) = &self.assets {
            state.serialize_field("assets", assets)?;
        }
        if let Some(requirements) = &self.requirements {
            state.serialize_field("requirements", requirements)?;
        }
        state.end()
    }
}

/// A reusable typed value and optional editor presentation metadata.
///
/// The JSON shape places `type` and `value` alongside the metadata. Metadata does not affect
/// materialization or confer a rendering capability.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDesignTokenV1 {
    /// Optional human-readable editor label.
    pub label: Option<String>,
    /// Optional explanatory editor text.
    pub description: Option<String>,
    /// Optional editor display group, independent of the token identifier.
    pub group: Option<String>,
    /// The token's declared type and direct value or same-type alias.
    pub value: ThemeDesignTokenValueV1,
}

/// The finite vocabulary of reusable theme values.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeDesignTokenValueV1 {
    /// A color string.
    Color(ThemeValueV1<String>),
    /// A dimensionless number, validated at each consumer.
    Number(ThemeValueV1<f32>),
    /// A string, constrained to a consumer's accepted choices where applicable.
    String(ThemeValueV1<String>),
    /// A pixel or percentage length, constrained by the consumer.
    Length(ThemeValueV1<ThemeLengthWireV1>),
    /// Pixel insets in the compact or four-sided shape.
    Insets(ThemeValueV1<ThemeInsetsWireV1>),
    /// A line-height keyword, multiplier, or pixel value.
    LineHeight(ThemeValueV1<ThemeLineHeightWireV1>),
    /// An ordered font-family stack.
    FontStack(ThemeValueV1<Vec<String>>),
    /// An ordered color palette whose entries may reference color tokens.
    Palette(ThemeValueV1<Vec<ThemeValueV1<String>>>),
    /// A solid, gradient, or pattern paint.
    Paint(ThemeValueV1<ThemeDesignPaintV1>),
    /// A stroke patch.
    Stroke(ThemeValueV1<ThemeDesignStrokeV1>),
    /// A text-style patch.
    TextStyle(ThemeValueV1<ThemeDesignTextStyleV1>),
    /// A shadow stack or explicit effect graph.
    Effect(ThemeValueV1<ThemeDesignEffectV1>),
}

impl Serialize for ThemeDesignTokenV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.value.contains_non_finite_number() {
            return Err(S::Error::custom("theme token contains a non-finite number"));
        }
        let field_count = 2
            + usize::from(self.label.is_some())
            + usize::from(self.description.is_some())
            + usize::from(self.group.is_some());
        let mut state = serializer.serialize_struct("ThemeDesignTokenV1", field_count)?;
        if let Some(label) = &self.label {
            state.serialize_field("label", label)?;
        }
        if let Some(description) = &self.description {
            state.serialize_field("description", description)?;
        }
        if let Some(group) = &self.group {
            state.serialize_field("group", group)?;
        }
        macro_rules! serialize_value {
            ($kind:literal, $value:expr) => {{
                state.serialize_field("type", $kind)?;
                state.serialize_field("value", $value)?;
            }};
        }
        match &self.value {
            ThemeDesignTokenValueV1::Color(value) => serialize_value!("color", value),
            ThemeDesignTokenValueV1::Number(value) => serialize_value!("number", value),
            ThemeDesignTokenValueV1::String(value) => serialize_value!("string", value),
            ThemeDesignTokenValueV1::Length(value) => serialize_value!("length", value),
            ThemeDesignTokenValueV1::Insets(value) => serialize_value!("insets", value),
            ThemeDesignTokenValueV1::LineHeight(value) => serialize_value!("line-height", value),
            ThemeDesignTokenValueV1::FontStack(value) => serialize_value!("font-stack", value),
            ThemeDesignTokenValueV1::Palette(value) => serialize_value!("palette", value),
            ThemeDesignTokenValueV1::Paint(value) => serialize_value!("paint", value),
            ThemeDesignTokenValueV1::Stroke(value) => serialize_value!("stroke", value),
            ThemeDesignTokenValueV1::TextStyle(value) => serialize_value!("text-style", value),
            ThemeDesignTokenValueV1::Effect(value) => serialize_value!("effect", value),
        }
        state.end()
    }
}

// The tagged bridge keeps metadata and the typed payload closed without combining serde's
// flattening and deny_unknown_fields attributes.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum ThemeDesignTokenDecodeV1 {
    Color(ThemeDesignTokenFieldsV1<String>),
    Number(ThemeDesignTokenFieldsV1<f32>),
    String(ThemeDesignTokenFieldsV1<String>),
    Length(ThemeDesignTokenFieldsV1<ThemeLengthWireV1>),
    Insets(ThemeDesignTokenFieldsV1<ThemeInsetsWireV1>),
    LineHeight(ThemeDesignTokenFieldsV1<ThemeLineHeightWireV1>),
    FontStack(ThemeDesignTokenFieldsV1<Vec<String>>),
    Palette(ThemeDesignTokenFieldsV1<Vec<ThemeValueV1<String>>>),
    Paint(ThemeDesignTokenFieldsV1<ThemeDesignPaintV1>),
    Stroke(ThemeDesignTokenFieldsV1<ThemeDesignStrokeV1>),
    TextStyle(ThemeDesignTokenFieldsV1<ThemeDesignTextStyleV1>),
    Effect(ThemeDesignTokenFieldsV1<ThemeDesignEffectV1>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeDesignTokenFieldsV1<T> {
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    label: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    group: Option<String>,
    value: ThemeValueV1<T>,
}

impl<'de> Deserialize<'de> for ThemeDesignTokenV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        macro_rules! token {
            ($fields:expr, $variant:ident) => {{
                let fields = $fields;
                Self {
                    label: fields.label,
                    description: fields.description,
                    group: fields.group,
                    value: ThemeDesignTokenValueV1::$variant(fields.value),
                }
            }};
        }
        Ok(match ThemeDesignTokenDecodeV1::deserialize(deserializer)? {
            ThemeDesignTokenDecodeV1::Color(fields) => token!(fields, Color),
            ThemeDesignTokenDecodeV1::Number(fields) => token!(fields, Number),
            ThemeDesignTokenDecodeV1::String(fields) => token!(fields, String),
            ThemeDesignTokenDecodeV1::Length(fields) => token!(fields, Length),
            ThemeDesignTokenDecodeV1::Insets(fields) => token!(fields, Insets),
            ThemeDesignTokenDecodeV1::LineHeight(fields) => token!(fields, LineHeight),
            ThemeDesignTokenDecodeV1::FontStack(fields) => token!(fields, FontStack),
            ThemeDesignTokenDecodeV1::Palette(fields) => token!(fields, Palette),
            ThemeDesignTokenDecodeV1::Paint(fields) => token!(fields, Paint),
            ThemeDesignTokenDecodeV1::Stroke(fields) => token!(fields, Stroke),
            ThemeDesignTokenDecodeV1::TextStyle(fields) => token!(fields, TextStyle),
            ThemeDesignTokenDecodeV1::Effect(fields) => token!(fields, Effect),
        })
    }
}

/// A semantic target and optional existing family, variant, and ordinal selectors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignRuleV1 {
    /// Semantic target identifier.
    pub target: String,
    /// Optional diagram-family identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub family: Option<String>,
    /// Optional semantic variant identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<String>,
    /// Optional exact or cyclic ordinal selector.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub ordinal: Option<ThemeOrdinalSelectorWireV1>,
    /// The authored style patch.
    pub style: ThemeDesignStyleV1,
}

/// A reference-capable style patch with the existing omission and explicit-clear semantics.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignStyleV1 {
    /// Fill paint.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub fill: SpecifiedWireV1<ThemeValueV1<ThemeDesignPaintV1>>,
    /// Overall paint opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub opacity: SpecifiedWireV1<ThemeValueV1<f32>>,
    /// Fill-specific opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub fill_opacity: SpecifiedWireV1<ThemeValueV1<f32>>,
    /// Optional stroke patch or whole-stroke reference; `null` is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub stroke: Option<ThemeValueV1<ThemeDesignStrokeV1>>,
    /// Geometry radius, constrained to pixel lengths by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub radius: SpecifiedWireV1<ThemeValueV1<ThemeLengthWireV1>>,
    /// Padding in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub padding: SpecifiedWireV1<ThemeValueV1<ThemeInsetsWireV1>>,
    /// Optional text-style patch or whole-style reference; `null` is rejected.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub typography: Option<ThemeValueV1<ThemeDesignTextStyleV1>>,
    /// Atomic effect value or reference.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub effect: SpecifiedWireV1<ThemeValueV1<ThemeDesignEffectV1>>,
}

/// A reference-capable stroke patch.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignStrokeV1 {
    /// Stroke paint.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub paint: SpecifiedWireV1<ThemeValueV1<ThemeDesignPaintV1>>,
    /// Stroke width, constrained to pixel lengths by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub width: SpecifiedWireV1<ThemeValueV1<ThemeLengthWireV1>>,
    /// Stroke dash lengths in pixels.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub dasharray: SpecifiedWireV1<Vec<ThemeValueV1<f32>>>,
    /// Stroke line-cap identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub linecap: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Stroke line-join identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub linejoin: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Stroke-specific opacity.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub opacity: SpecifiedWireV1<ThemeValueV1<f32>>,
}

/// A reference-capable text-style patch.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignTextStyleV1 {
    /// Font-family stack.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_stack: SpecifiedWireV1<ThemeValueV1<Vec<String>>>,
    /// Font size, constrained to pixel lengths by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_size_px: SpecifiedWireV1<ThemeValueV1<ThemeLengthWireV1>>,
    /// Numeric font weight, constrained to the supported integer domain by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_weight: SpecifiedWireV1<ThemeValueV1<f32>>,
    /// Font-style identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub font_style: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Line-height keyword, unitless multiplier, or pixel value.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub line_height: SpecifiedWireV1<ThemeValueV1<ThemeLineHeightWireV1>>,
    /// Letter spacing, constrained to pixel lengths by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub letter_spacing_px: SpecifiedWireV1<ThemeValueV1<ThemeLengthWireV1>>,
    /// Word spacing, constrained to pixel lengths by its consumers.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub word_spacing_px: SpecifiedWireV1<ThemeValueV1<ThemeLengthWireV1>>,
    /// Text-transform identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub transform: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Text-decoration identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub decoration: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Text-alignment identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub text_align: SpecifiedWireV1<ThemeValueV1<String>>,
    /// White-space identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub white_space: SpecifiedWireV1<ThemeValueV1<String>>,
    /// Text-wrap identifier.
    #[serde(default, skip_serializing_if = "SpecifiedWireV1::is_unspecified")]
    pub wrap: SpecifiedWireV1<ThemeValueV1<String>>,
}

/// Default and family-specific authored typography.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignTypographyV1 {
    /// Optional default text-style patch or reference.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub default: Option<ThemeValueV1<ThemeDesignTextStyleV1>>,
    /// Family-specific text-style patches or references.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub families: BTreeMap<String, ThemeValueV1<ThemeDesignTextStyleV1>>,
}

/// A compact color or a structured authored paint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeDesignPaintV1 {
    /// Compact literal color, including `"transparent"`.
    Color(String),
    /// Structured paint with reference-capable fields.
    Structured(ThemeDesignPaintObjectV1),
}

/// Structured paints accepted by the design contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeDesignPaintObjectV1 {
    /// Fully transparent paint.
    Transparent,
    /// Solid paint with a color value or reference.
    Solid {
        /// Solid color.
        color: ThemeValueV1<String>,
    },
    /// A linear gradient.
    LinearGradient {
        /// Gradient angle in degrees.
        angle_degrees: ThemeValueV1<f32>,
        /// Color stops in source order.
        stops: Vec<ThemeDesignGradientStopV1>,
        /// Optional existing literal repetition configuration.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        repetition: Option<ThemeLinearGradientRepetitionWireV1>,
    },
    /// A radial gradient.
    RadialGradient {
        /// Horizontal center.
        center_x: ThemeValueV1<ThemeLengthWireV1>,
        /// Vertical center.
        center_y: ThemeValueV1<ThemeLengthWireV1>,
        /// Gradient radius.
        radius: ThemeValueV1<ThemeLengthWireV1>,
        /// Color stops in source order.
        stops: Vec<ThemeDesignGradientStopV1>,
        /// Optional existing literal repetition configuration.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        repetition: Option<ThemeRadialGradientRepetitionWireV1>,
    },
    /// A typed pattern request; acceptance does not establish renderer support.
    Pattern {
        /// Existing pattern-kind identifier.
        pattern: String,
        /// Pattern cell width, constrained to pixels by its consumers.
        cell_width: ThemeValueV1<ThemeLengthWireV1>,
        /// Pattern cell height, constrained to pixels by its consumers.
        cell_height: ThemeValueV1<ThemeLengthWireV1>,
        /// Foreground color.
        foreground: ThemeValueV1<String>,
        /// Optional background color.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        background: Option<ThemeValueV1<String>>,
        /// Optional pattern angle in degrees.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        angle_degrees: Option<ThemeValueV1<f32>>,
    },
}

/// A reference-capable color stop in an authored gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignGradientStopV1 {
    /// Stop position in the supported unit interval.
    pub offset: ThemeValueV1<f32>,
    /// Stop color.
    pub color: ThemeValueV1<String>,
    /// Optional multiplier applied to the resolved color's alpha.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub opacity: Option<ThemeValueV1<f32>>,
}

/// Authored canvas paint, ordered layers, and bleed.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignCanvasV1 {
    /// Optional canvas base paint or reference.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub base: Option<ThemeValueV1<ThemeDesignPaintV1>>,
    /// Canvas layers from back to front.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ThemeDesignCanvasLayerV1>,
    /// Optional pixel bleed.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub bleed: Option<ThemeValueV1<ThemeInsetsWireV1>>,
}

/// An ordered authored canvas layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignCanvasLayerV1 {
    /// Layer paint or reference.
    pub paint: ThemeValueV1<ThemeDesignPaintV1>,
    /// Optional layer opacity.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub opacity: Option<ThemeValueV1<f32>>,
    /// Optional existing blend-mode identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub blend_mode: Option<String>,
    /// Optional horizontal offset, constrained to pixels by its consumers.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset_x: Option<ThemeValueV1<ThemeLengthWireV1>>,
    /// Optional vertical offset, constrained to pixels by its consumers.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset_y: Option<ThemeValueV1<ThemeLengthWireV1>>,
}

/// Explicit composition of the shadows in an authored stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeShadowCompositionV1 {
    /// Every shadow reads the original terminal; shadows paint in order below the source.
    Independent,
    /// Each shadow reads the previous complete result, matching sequential drop shadows.
    Cascade,
}

/// An authored shadow stack or an advanced literal graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeDesignEffectV1 {
    /// Common hard shadows, soft shadows, and glows with explicit composition.
    ShadowStack {
        /// Whether shadows read the original or accumulated terminal.
        composition: ThemeShadowCompositionV1,
        /// Optional interpolation space; the new convenience defaults to sRGB.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        color_space: Option<String>,
        /// Shadows in authored paint order.
        shadows: Vec<ThemeDesignShadowV1>,
    },
    /// An explicit existing effect graph, without a public lowered graph identifier.
    Graph {
        /// Optional existing graph interpolation space.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        color_space: Option<String>,
        /// Existing literal primitives in authored order.
        primitives: Vec<ThemeEffectPrimitiveWireV1>,
    },
}

/// One authored shadow; inset and spread are deliberately unsupported convenience fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(remote = "Self")]
pub struct ThemeDesignShadowV1 {
    /// Signed horizontal diagram-space pixel offset.
    pub offset_x: ThemeValueV1<ThemeLengthWireV1>,
    /// Signed vertical diagram-space pixel offset.
    pub offset_y: ThemeValueV1<ThemeLengthWireV1>,
    /// Gaussian standard deviation in pixels; zero produces a hard shadow.
    pub std_deviation: ThemeValueV1<ThemeLengthWireV1>,
    /// Shadow color.
    pub color: ThemeValueV1<String>,
    /// Optional multiplier applied to the resolved color's alpha.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub opacity: Option<ThemeValueV1<f32>>,
}

impl ContainsNonFiniteNumber for String {
    fn contains_non_finite_number(&self) -> bool {
        false
    }
}

impl<T: ContainsNonFiniteNumber> ContainsNonFiniteNumber for ThemeValueV1<T> {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Ref(_) => false,
            Self::Literal(value) => value.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeDesignV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.tokens.contains_non_finite_number()
            || self.styles.contains_non_finite_number()
            || self.rules.contains_non_finite_number()
            || self.palettes.contains_non_finite_number()
            || self.canvas.contains_non_finite_number()
            || self.typography.contains_non_finite_number()
            || self.mermaid.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignTokenV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.value.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignTokenValueV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Color(_) | Self::String(_) | Self::FontStack(_) | Self::Palette(_) => false,
            Self::Number(value) => value.contains_non_finite_number(),
            Self::Length(value) => value.contains_non_finite_number(),
            Self::Insets(value) => value.contains_non_finite_number(),
            Self::LineHeight(value) => value.contains_non_finite_number(),
            Self::Paint(value) => value.contains_non_finite_number(),
            Self::Stroke(value) => value.contains_non_finite_number(),
            Self::TextStyle(value) => value.contains_non_finite_number(),
            Self::Effect(value) => value.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeDesignRuleV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.style.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignStyleV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.fill.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
            || self.fill_opacity.contains_non_finite_number()
            || self.stroke.contains_non_finite_number()
            || self.radius.contains_non_finite_number()
            || self.padding.contains_non_finite_number()
            || self.typography.contains_non_finite_number()
            || self.effect.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignStrokeV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.paint.contains_non_finite_number()
            || self.width.contains_non_finite_number()
            || self.dasharray.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignTextStyleV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.font_size_px.contains_non_finite_number()
            || self.font_weight.contains_non_finite_number()
            || self.line_height.contains_non_finite_number()
            || self.letter_spacing_px.contains_non_finite_number()
            || self.word_spacing_px.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignTypographyV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.default.contains_non_finite_number() || self.families.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignPaintV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Color(_) => false,
            Self::Structured(paint) => paint.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeDesignPaintObjectV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Transparent | Self::Solid { .. } => false,
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
                cell_width,
                cell_height,
                angle_degrees,
                ..
            } => {
                cell_width.contains_non_finite_number()
                    || cell_height.contains_non_finite_number()
                    || angle_degrees.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeDesignGradientStopV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.offset.contains_non_finite_number() || self.opacity.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignCanvasV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.base.contains_non_finite_number()
            || self.layers.contains_non_finite_number()
            || self.bleed.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignCanvasLayerV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.paint.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
            || self.offset_x.contains_non_finite_number()
            || self.offset_y.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeDesignEffectV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::ShadowStack { shadows, .. } => shadows.contains_non_finite_number(),
            Self::Graph { primitives, .. } => primitives.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeDesignShadowV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.offset_x.contains_non_finite_number()
            || self.offset_y.contains_non_finite_number()
            || self.std_deviation.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
    }
}

/// The finite consumer domain for a reusable authoring value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(remote = "Self")]
pub struct ThemeDesignValueDomain {
    /// Expected reusable-value type.
    pub value_type: String,
    /// Admitted finite string choices, when constrained.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    /// Admitted length units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<String>,
    /// Inclusive numeric minimum, when constrained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    /// Inclusive numeric maximum, when constrained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
}

/// One authored reference, independent of which rule wins at execution time.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(remote = "Self")]
pub struct ThemeDesignReferenceUse {
    /// Exact document-local token identifier.
    pub token: String,
    /// JSON Pointer to the authored consumer.
    pub path: String,
    /// Consumer-owned units, choices, and bounds.
    pub domain: ThemeDesignValueDomain,
}

/// On-demand inspection of declared authoring bindings.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(remote = "Self")]
pub struct ThemeDesignInspection {
    /// Declared references and consumer domains in deterministic traversal order.
    pub references: Vec<ThemeDesignReferenceUse>,
}

object_deserialize!(
    ThemeDesignV1,
    ThemeDesignRuleV1,
    ThemeDesignStyleV1,
    ThemeDesignStrokeV1,
    ThemeDesignTextStyleV1,
    ThemeDesignTypographyV1,
    ThemeDesignGradientStopV1,
    ThemeDesignCanvasV1,
    ThemeDesignCanvasLayerV1,
    ThemeDesignShadowV1,
    ThemeDesignValueDomain,
    ThemeDesignReferenceUse,
    ThemeDesignInspection
);
object_serialize!(
    ThemeDesignRuleV1,
    ThemeDesignStyleV1,
    ThemeDesignStrokeV1,
    ThemeDesignTextStyleV1,
    ThemeDesignTypographyV1,
    ThemeDesignGradientStopV1,
    ThemeDesignCanvasV1,
    ThemeDesignCanvasLayerV1,
    ThemeDesignShadowV1,
    ThemeDesignValueDomain,
    ThemeDesignReferenceUse,
    ThemeDesignInspection
);
