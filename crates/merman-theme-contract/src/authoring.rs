use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonical_json::{CanonicalJsonError, canonical_json_bytes};
use crate::finite::ContainsNonFiniteNumber;
use crate::version::THEME_CONTRACT_V1;
use crate::wire::{ThemeRuleSetWireV1, deserialize_optional_non_null};

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
        self.tokens.contains_non_finite_number() || self.styles.contains_non_finite_number()
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
    /// The border color.
    Border,
    /// The line color.
    Line,
    /// The accent color.
    Accent,
}

impl ThemeColorTokenV1 {
    /// Every version 1 color token in contract order.
    pub const ALL: [Self; 8] = [
        Self::Canvas,
        Self::Surface,
        Self::SurfaceAlt,
        Self::SurfaceMuted,
        Self::Text,
        Self::Border,
        Self::Line,
        Self::Accent,
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
            ThemeColorTokenV1::Border => &mut self.border,
            ThemeColorTokenV1::Line => &mut self.line,
            ThemeColorTokenV1::Accent => &mut self.accent,
        };
        *slot = Some(value.into());
        self
    }

    /// Sets the diagram canvas color.
    pub fn with_canvas(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Canvas, value)
    }

    /// Sets the primary surface color.
    pub fn with_surface(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Surface, value)
    }

    /// Sets the alternate surface color.
    pub fn with_surface_alt(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::SurfaceAlt, value)
    }

    /// Sets the muted surface color.
    pub fn with_surface_muted(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::SurfaceMuted, value)
    }

    /// Sets the primary text color.
    pub fn with_text(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Text, value)
    }

    /// Sets the border color.
    pub fn with_border(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Border, value)
    }

    /// Sets the line color.
    pub fn with_line(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Line, value)
    }

    /// Sets the accent color.
    pub fn with_accent(self, value: impl Into<String>) -> Self {
        self.with_color(ThemeColorTokenV1::Accent, value)
    }

    /// Returns one authored color token, or `None` when it was omitted.
    pub fn color(&self, token: ThemeColorTokenV1) -> Option<&str> {
        match token {
            ThemeColorTokenV1::Canvas => self.canvas.as_deref(),
            ThemeColorTokenV1::Surface => self.surface.as_deref(),
            ThemeColorTokenV1::SurfaceAlt => self.surface_alt.as_deref(),
            ThemeColorTokenV1::SurfaceMuted => self.surface_muted.as_deref(),
            ThemeColorTokenV1::Text => self.text.as_deref(),
            ThemeColorTokenV1::Border => self.border.as_deref(),
            ThemeColorTokenV1::Line => self.line.as_deref(),
            ThemeColorTokenV1::Accent => self.accent.as_deref(),
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
}

impl ContainsNonFiniteNumber for ThemeAuthoringTypographyV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.font_size_px.contains_non_finite_number()
    }
}
