use std::collections::BTreeMap;

use serde::ser::Error as _;
use serde::{Deserialize, Serialize, Serializer};

use crate::canonical_json::{CanonicalJsonError, canonical_json_bytes};
use crate::finite::ContainsNonFiniteNumber;
use crate::version::THEME_CONTRACT_V1;
use crate::wire::{
    ThemeCanvasPaintWireV1, ThemeInsetsWireV1, ThemeLineHeightWireV1, ThemeRuleSetWireV1,
    deserialize_optional_non_null,
};

/// The closed complete-theme-spec wire for schema version 1.
///
/// This type preserves wire presence and raw values. Semantic validation and defaulting belong to
/// the renderer-owned decoder.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramThemeSpecWireV1 {
    /// Optional Mermaid compatibility values.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub mermaid: Option<MermaidThemeCompatibilityWireV1>,
    /// Optional complete typography configuration.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub typography: Option<ThemeTypographySpecWireV1>,
    /// Optional flat rule-set entries; an explicit empty array is preserved.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub styles: Option<Vec<ThemeRuleSetWireV1>>,
    /// Optional canvas configuration.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub canvas: Option<ThemeCanvasSpecWireV1>,
    /// Optional effect entries; an explicit empty array is preserved.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub effects: Option<Vec<ThemeEffectEntryWireV1>>,
    /// Optional declared capability requirements.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub requirements: Option<ThemeRequirementsWireV1>,
    /// Optional embedded theme assets.
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub assets: Option<ThemeAssetsWireV1>,
}

impl DiagramThemeSpecWireV1 {
    /// Returns the complete-spec schema version owned by the contract registry.
    pub const fn spec_schema_version() -> u32 {
        THEME_CONTRACT_V1.spec_schema_version()
    }

    /// Serializes this complete spec using RFC 8785 JSON Canonicalization Scheme.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, CanonicalJsonError> {
        if self.contains_non_finite_number() {
            return Err(CanonicalJsonError::invalid_input(
                "complete theme spec contains a non-finite number",
            ));
        }
        canonical_json_bytes(&self.encode())
    }

    const fn encode(&self) -> DiagramThemeSpecWireV1Encode<'_> {
        DiagramThemeSpecWireV1Encode {
            mermaid: &self.mermaid,
            typography: &self.typography,
            styles: &self.styles,
            canvas: &self.canvas,
            effects: &self.effects,
            requirements: &self.requirements,
            assets: &self.assets,
        }
    }
}

#[derive(Serialize)]
struct DiagramThemeSpecWireV1Encode<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    mermaid: &'a Option<MermaidThemeCompatibilityWireV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    typography: &'a Option<ThemeTypographySpecWireV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    styles: &'a Option<Vec<ThemeRuleSetWireV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canvas: &'a Option<ThemeCanvasSpecWireV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effects: &'a Option<Vec<ThemeEffectEntryWireV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requirements: &'a Option<ThemeRequirementsWireV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assets: &'a Option<ThemeAssetsWireV1>,
}

impl Serialize for DiagramThemeSpecWireV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.contains_non_finite_number() {
            return Err(S::Error::custom(
                "complete theme spec contains a non-finite number",
            ));
        }
        self.encode().serialize(serializer)
    }
}

/// Restricted scalar Mermaid compatibility values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MermaidThemeValueWireV1 {
    /// A string value.
    String(String),
    /// A numeric value.
    Number(f64),
    /// A boolean value.
    Boolean(bool),
}

/// The closed Mermaid compatibility object.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MermaidThemeCompatibilityWireV1 {
    /// Optional upstream Mermaid theme identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub theme: Option<String>,
    /// Optional mirrored dark-mode value.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub dark_mode: Option<bool>,
    /// Optional compatibility variable map.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub variables: Option<BTreeMap<String, MermaidThemeValueWireV1>>,
}

/// Complete default and family-specific typography.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTypographySpecWireV1 {
    /// Optional default text style.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub default: Option<ThemeTextStyleWireV1>,
    /// Optional family-specific text styles keyed by raw family identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub families: Option<BTreeMap<String, ThemeTextStyleWireV1>>,
}

/// A complete text-style wire whose omitted fields retain renderer defaults.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeTextStyleWireV1 {
    /// Optional font-family stack.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub font_stack: Option<Vec<String>>,
    /// Optional font size in pixels.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub font_size_px: Option<f32>,
    /// Optional numeric font weight.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub font_weight: Option<u16>,
    /// Optional raw font-style identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub font_style: Option<String>,
    /// Optional line height.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub line_height: Option<ThemeLineHeightWireV1>,
    /// Optional letter spacing in pixels.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub letter_spacing_px: Option<f32>,
    /// Optional word spacing in pixels.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub word_spacing_px: Option<f32>,
    /// Optional raw text-transform identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub transform: Option<String>,
    /// Optional raw text-decoration identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub decoration: Option<String>,
    /// Optional raw text-alignment identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_align: Option<String>,
    /// Optional raw white-space identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub white_space: Option<String>,
    /// Optional raw wrap-mode identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub wrap: Option<String>,
}

/// A complete canvas recipe.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeCanvasSpecWireV1 {
    /// Optional base paint.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub base: Option<ThemeCanvasPaintWireV1>,
    /// Optional ordered canvas layers; an explicit empty array is preserved.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub layers: Option<Vec<ThemeCanvasLayerWireV1>>,
    /// Optional canvas bleed.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub bleed: Option<ThemeInsetsWireV1>,
}

/// One ordered canvas layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeCanvasLayerWireV1 {
    /// Layer paint.
    pub paint: ThemeCanvasPaintWireV1,
    /// Optional opacity.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub opacity: Option<f32>,
    /// Optional raw blend-mode identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub blend_mode: Option<String>,
    /// Optional horizontal offset in pixels.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset_x: Option<f32>,
    /// Optional vertical offset in pixels.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset_y: Option<f32>,
}

/// A graph or semantic binding in the effect-set wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeEffectEntryWireV1 {
    /// An ordered effect graph.
    Graph {
        /// Recipe-local graph identifier.
        id: String,
        /// Optional interpolation space: `linear-rgb` (default) or `srgb`.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        color_space: Option<String>,
        /// Ordered effect primitives.
        primitives: Vec<ThemeEffectPrimitiveWireV1>,
    },
    /// A semantic target binding.
    Binding {
        /// Raw semantic target identifier.
        target: String,
        /// Referenced graph identifier.
        effect_id: String,
    },
}

/// One primitive in an effect graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ThemeEffectPrimitiveWireV1 {
    /// A drop shadow.
    DropShadow {
        /// Optional raw effect input identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        input: Option<String>,
        /// Horizontal offset.
        offset_x: f32,
        /// Vertical offset.
        offset_y: f32,
        /// Blur radius.
        blur_radius: f32,
        /// Shadow spread.
        spread: f32,
        /// Raw shadow color.
        color: String,
    },
    /// A Gaussian blur.
    GaussianBlur {
        /// Optional raw effect input identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        input: Option<String>,
        /// Standard deviation.
        std_deviation: f32,
    },
    /// A color matrix.
    ColorMatrix {
        /// Optional raw effect input identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        input: Option<String>,
        /// Raw matrix values; the renderer decoder validates the required count.
        values: Vec<f32>,
    },
    /// Procedural turbulence.
    Turbulence {
        /// Optional raw effect input identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        input: Option<String>,
        /// Horizontal base frequency.
        base_frequency_x: f32,
        /// Vertical base frequency.
        base_frequency_y: f32,
        /// Raw octave count.
        octaves: u8,
        /// Raw deterministic seed.
        seed: i32,
    },
    /// A displacement map.
    Displacement {
        /// Optional raw effect input identifier.
        #[serde(
            default,
            deserialize_with = "deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        input: Option<String>,
        /// Raw map-input identifier.
        map_input: String,
        /// Displacement scale.
        scale: f32,
    },
}

/// Declared complete-spec capability requirements.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeRequirementsWireV1 {
    /// Optional raw theme capability identifiers.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub capabilities: Option<Vec<String>>,
    /// Optional raw text-layout capability identifiers.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_capabilities: Option<Vec<String>>,
}

/// Embedded resources and font-policy declarations carried by a complete spec.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeAssetsWireV1 {
    /// Optional embedded font assets.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub fonts: Option<Vec<ThemeFontAssetWireV1>>,
    /// Optional font-family aliases.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub aliases: Option<Vec<ThemeFontAliasWireV1>>,
    /// Optional CSS generic-family mappings.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub generic_families: Option<Vec<ThemeGenericFamilyWireV1>>,
    /// Optional raw available font-source identifiers.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub available_sources: Option<Vec<String>>,
    /// Optional raw font-embedding requirement identifier.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub embedding: Option<String>,
}

/// One base64-encoded font asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFontAssetWireV1 {
    /// Recipe-local asset identifier.
    pub id: String,
    /// Raw declared font-container identifier.
    pub format: String,
    /// Raw base64 payload; the renderer-owned decoder validates canonical encoding.
    pub data_base64: String,
}

/// One font-family alias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFontAliasWireV1 {
    /// Alias name.
    pub alias: String,
    /// Target family name.
    pub target: String,
}

/// One CSS generic-family mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeGenericFamilyWireV1 {
    /// Raw CSS generic-family identifier.
    pub generic: String,
    /// Target family name.
    pub target: String,
}

impl ContainsNonFiniteNumber for DiagramThemeSpecWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            mermaid,
            typography,
            styles,
            canvas,
            effects,
            requirements: _,
            assets: _,
        } = self;

        mermaid.contains_non_finite_number()
            || typography.contains_non_finite_number()
            || styles.contains_non_finite_number()
            || canvas.contains_non_finite_number()
            || effects.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for MermaidThemeCompatibilityWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            theme: _,
            dark_mode: _,
            variables,
        } = self;

        variables.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for MermaidThemeValueWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::String(_) | Self::Boolean(_) => false,
            Self::Number(value) => value.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeTypographySpecWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self { default, families } = self;

        default.contains_non_finite_number() || families.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeTextStyleWireV1 {
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

impl ContainsNonFiniteNumber for ThemeCanvasSpecWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            base,
            layers,
            bleed,
        } = self;

        base.contains_non_finite_number()
            || layers.contains_non_finite_number()
            || bleed.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeCanvasLayerWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        let Self {
            paint,
            opacity,
            blend_mode: _,
            offset_x,
            offset_y,
        } = self;

        paint.contains_non_finite_number()
            || opacity.contains_non_finite_number()
            || offset_x.contains_non_finite_number()
            || offset_y.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeEffectEntryWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Graph { primitives, .. } => primitives.contains_non_finite_number(),
            Self::Binding {
                target: _,
                effect_id: _,
            } => false,
        }
    }
}

impl ContainsNonFiniteNumber for ThemeEffectPrimitiveWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::DropShadow {
                input: _,
                offset_x,
                offset_y,
                blur_radius,
                spread,
                color: _,
            } => {
                offset_x.contains_non_finite_number()
                    || offset_y.contains_non_finite_number()
                    || blur_radius.contains_non_finite_number()
                    || spread.contains_non_finite_number()
            }
            Self::GaussianBlur {
                input: _,
                std_deviation,
            } => std_deviation.contains_non_finite_number(),
            Self::ColorMatrix { input: _, values } => values.contains_non_finite_number(),
            Self::Turbulence {
                input: _,
                base_frequency_x,
                base_frequency_y,
                octaves: _,
                seed: _,
            } => {
                base_frequency_x.contains_non_finite_number()
                    || base_frequency_y.contains_non_finite_number()
            }
            Self::Displacement {
                input: _,
                map_input: _,
                scale,
            } => scale.contains_non_finite_number(),
        }
    }
}
