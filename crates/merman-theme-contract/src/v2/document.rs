use std::fmt;

#[cfg(feature = "compile")]
use serde::Deserializer;
#[cfg(feature = "compile")]
use serde::de::{self, IntoDeserializer, value::MapAccessDeserializer};
use serde::{Deserialize, Serialize};

use super::Color;
#[cfg(feature = "compile")]
use super::json::{object, objects, present_object};

// Optional fields in the document are omitted, so JSON null cannot be mistaken for an override.
#[cfg(feature = "compile")]
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[cfg(feature = "compile")]
macro_rules! patch {
    ($name:ident { $($(#[$attr:meta])* $field:ident : $kind:ty),* $(,)? }
        $(objects { $($object:ident : $object_kind:ty),+ $(,)? })?) => {
        #[derive(Clone, Debug, Default, Deserialize, PartialEq)]
        #[serde(default, deny_unknown_fields)]
        pub(super) struct $name {
            $(
                $(#[$attr])*
                #[serde(deserialize_with = "present")]
                pub(super) $field: Option<$kind>,
            )*
            $($(
                #[serde(deserialize_with = "present_object")]
                pub(super) $object: Option<$object_kind>,
            )+)?
        }
    };
}

// Decode fields on the input stream while allowing `kind` to appear anywhere in the object.
#[cfg(feature = "compile")]
macro_rules! tagged_fields {
    ($name:ident { kind: $tag:ty, $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $name {
            kind: $tag,
            $(
                #[serde(default, deserialize_with = "present")]
                $field: Option<$ty>,
            )+
        }

        impl $name {
            fn reject_unused<E: de::Error>(&self) -> Result<(), E> {
                $(
                    if self.$field.is_some() {
                        return Err(E::custom(concat!(
                            "unknown field `", stringify!($field), "` for the selected kind"
                        )));
                    }
                )+
                Ok(())
            }
        }
    };
}

#[cfg(feature = "compile")]
fn take_required<T, E: de::Error>(field: &mut Option<T>, name: &'static str) -> Result<T, E> {
    field.take().ok_or_else(|| E::missing_field(name))
}

#[cfg(feature = "compile")]
macro_rules! overlay {
    ($target:expr, $patch:expr, $($field:ident),+ $(,)?) => {
        $(if let Some(value) = &$patch.$field {
            $target.$field.clone_from(value);
        })+
    };
}

#[cfg(feature = "compile")]
macro_rules! merge {
    ($target:expr, $patch:expr, $($field:ident),+ $(,)?) => {
        $(if $patch.$field.is_some() {
            $target.$field.clone_from(&$patch.$field);
        })+
    };
}

/// The concrete light or dark appearance selected by the host.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    /// Light surfaces.
    Light,
    /// Dark surfaces.
    Dark,
}

impl Appearance {
    /// The spelling used by theme documents and tools.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl fmt::Display for Appearance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A theme's catalog grouping.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// General-purpose reading.
    Reading,
    /// Editor-inspired design.
    Editor,
    /// Material or illustrative design.
    Art,
    /// User-authored design.
    Custom,
}

/// A version 2 theme document. Use [`super::ThemeCompiler`] to admit and resolve it.
#[cfg(feature = "compile")]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct ThemeDocumentV2 {
    #[serde(deserialize_with = "object")]
    pub(super) fields: ThemeDocumentFields,
}

#[cfg(feature = "compile")]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ThemeDocumentFields {
    #[serde(rename = "$schema", default, deserialize_with = "present")]
    schema: Option<String>,
    pub(super) schema_version: u32,
    #[serde(default, deserialize_with = "present")]
    pub(super) id: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) description: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) category: Option<Category>,
    #[serde(default, deserialize_with = "present")]
    pub(super) extends: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) default_appearance: Option<Appearance>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) seed: Option<SeedPatch>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) roles: Option<RoleOverrides>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) typography: Option<TypographyPatch>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) form: Option<FormPatch>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) appearances: Option<Appearances>,
    #[serde(default, deserialize_with = "objects")]
    pub(super) overrides: Vec<FamilyOverride>,
}

#[cfg(feature = "compile")]
impl ThemeDocumentV2 {
    /// The document's own identifier, when supplied.
    pub fn id(&self) -> Option<&str> {
        self.fields.id.as_deref()
    }

    /// The display name supplied by the author.
    pub fn name(&self) -> Option<&str> {
        self.fields.name.as_deref()
    }

    /// A short description of this design.
    pub fn description(&self) -> Option<&str> {
        self.fields.description.as_deref()
    }

    /// The design's catalog category.
    pub const fn category(&self) -> Option<Category> {
        self.fields.category
    }

    /// The built-in document this theme extends, when supplied.
    pub fn extends(&self) -> Option<&str> {
        self.fields.extends.as_deref()
    }

    /// Light and dark layers declared in this document, before preset extension.
    pub fn appearances(&self) -> impl Iterator<Item = Appearance> + '_ {
        self.fields.appearances()
    }
}

#[cfg(feature = "compile")]
impl ThemeDocumentFields {
    pub(super) fn appearances(&self) -> impl Iterator<Item = Appearance> + '_ {
        [Appearance::Light, Appearance::Dark]
            .into_iter()
            .filter(|appearance| self.layer(*appearance).is_some())
    }

    pub(super) fn layer(&self, appearance: Appearance) -> Option<&AppearanceLayer> {
        let appearances = self.appearances.as_ref()?;
        match appearance {
            Appearance::Light => appearances.light.as_ref(),
            Appearance::Dark => appearances.dark.as_ref(),
        }
    }
}

#[cfg(feature = "compile")]
patch!(Appearances {} objects {
    light: AppearanceLayer,
    dark: AppearanceLayer
});
#[cfg(feature = "compile")]
patch!(AppearanceLayer {
    elevation: ElevationSpec,
} objects {
    seed: SeedPatch,
    roles: RoleOverrides,
    data: DataPatch,
    canvas: CanvasPatch,
});
#[cfg(feature = "compile")]
patch!(SeedPatch {
    canvas: Color,
    ink: Color,
    accent: Color,
    surface: Color,
    callout: Color,
    info: Color,
    success: Color,
    warn: Color,
    danger: Color,
});

#[cfg(feature = "compile")]
impl SeedPatch {
    pub(super) fn merge(&mut self, patch: &Self) {
        merge!(
            self, patch, canvas, ink, accent, surface, callout, info, success, warn, danger
        );
    }
}

macro_rules! role_fields {
    ($($(#[$doc:meta])* $field:ident : $kind:ty = $key:literal),+ $(,)?) => {
        /// Concrete semantic paints and colors shared by diagram families.
        #[derive(Clone, Debug, PartialEq, Serialize)]
        #[non_exhaustive]
        pub struct Roles {
            $($(#[$doc])* #[serde(rename = $key)] pub $field: $kind,)+
        }

        #[cfg(feature = "compile")]
        patch!(RoleOverrides { $(#[serde(rename = $key)] $field: $kind,)+ });

        #[cfg(feature = "compile")]
        impl Roles {
            pub(super) fn apply(&mut self, patch: &RoleOverrides) {
                overlay!(self, patch, $($field),+);
            }
        }
    };
}

role_fields! {
    /// Diagram canvas paint.
    canvas: Paint = "canvas",
    /// Ordinary node surface.
    surface_node: Paint = "surface.node",
    /// Group or enclosure surface.
    surface_container: Paint = "surface.container",
    /// Note or callout surface.
    surface_callout: Paint = "surface.callout",
    /// Emphasis surface.
    surface_emphasis: Paint = "surface.emphasis",
    /// Structure outline.
    stroke_border: Paint = "stroke.border",
    /// Connection line.
    stroke_connector: Paint = "stroke.connector",
    /// Secondary guide line.
    stroke_guide: Paint = "stroke.guide",
    /// Primary text.
    text_primary: Color = "text.primary",
    /// Secondary text.
    text_muted: Color = "text.muted",
    /// Title or accent text.
    text_title: Color = "text.title",
    /// Text over the callout surface.
    text_callout: Color = "text.callout",
    /// Text over the emphasis surface.
    text_on_emphasis: Color = "text.on_emphasis",
    /// Informational state.
    status_info: Color = "status.info",
    /// Successful state.
    status_success: Color = "status.success",
    /// Warning state.
    status_warn: Color = "status.warn",
    /// Error or critical state.
    status_danger: Color = "status.danger",
}

#[cfg(feature = "compile")]
patch!(DataPatch {
    ink: Vec<Color>,
    fill: Vec<Color>,
    fill_ink: FillInk,
    pattern_encoding: bool,
});

#[cfg(feature = "compile")]
impl DataPatch {
    pub(super) fn merge(&mut self, patch: &Self) {
        merge!(self, patch, ink, fill, fill_ink, pattern_encoding);
    }
}

#[cfg(feature = "compile")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FillInk {
    Auto(Auto),
    Color(Color),
}

#[cfg(feature = "compile")]
impl<'de> Deserialize<'de> for FillInk {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FillInkVisitor;

        impl de::Visitor<'_> for FillInkVisitor {
            type Value = FillInk;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("`auto` or a #RRGGBB or #RRGGBBAA color string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<FillInk, E> {
                if value == "auto" {
                    return Ok(FillInk::Auto(Auto::Auto));
                }
                Color::parse(value).map(FillInk::Color).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(FillInkVisitor)
    }
}

#[cfg(feature = "compile")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Auto {
    Auto,
}

/// A solid color or a bounded linear gradient.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Paint {
    /// A solid color, which can be transparent.
    Solid(Color),
    /// A linear gradient with ordered stops.
    LinearGradient(LinearGradient),
}

impl From<Color> for Paint {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

impl Paint {
    /// Returns the starting color, including stop opacity, for single-color surfaces.
    pub fn first_color(&self) -> Color {
        match self {
            Self::Solid(color) => *color,
            Self::LinearGradient(gradient) => {
                gradient.stops.first().map_or(Color::TRANSPARENT, |stop| {
                    stop.color.composite(Color::TRANSPARENT, stop.opacity)
                })
            }
        }
    }
}

#[cfg(feature = "compile")]
impl<'de> Deserialize<'de> for Paint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PaintVisitor;

        impl<'de> de::Visitor<'de> for PaintVisitor {
            type Value = Paint;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a color string or a linear gradient object")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Paint, E> {
                if value == "transparent" {
                    return Ok(Paint::Solid(Color::TRANSPARENT));
                }
                Color::parse(value).map(Paint::Solid).map_err(E::custom)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Paint, A::Error> {
                LinearGradient::deserialize(MapAccessDeserializer::new(map))
                    .map(Paint::LinearGradient)
            }
        }

        deserializer.deserialize_any(PaintVisitor)
    }
}

/// A stop in a linear gradient.
#[cfg_attr(feature = "compile", derive(Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "compile", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct GradientStop {
    /// Gradient position between zero and one.
    pub offset: f64,
    /// Stop color.
    pub color: Color,
    /// Opacity applied in addition to the color's alpha.
    #[cfg_attr(feature = "compile", serde(default = "opaque"))]
    pub opacity: f64,
}

/// A linear gradient along an axis specified in degrees.
#[cfg_attr(feature = "compile", derive(Deserialize))]
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "compile", serde(deny_unknown_fields))]
pub struct LinearGradient {
    kind: GradientKind,
    /// Angle in degrees.
    #[cfg_attr(feature = "compile", serde(default))]
    pub angle_deg: f64,
    /// Stops in nondecreasing offset order.
    #[cfg_attr(feature = "compile", serde(deserialize_with = "objects"))]
    pub stops: Vec<GradientStop>,
}

#[cfg_attr(feature = "compile", derive(Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    not(feature = "compile"),
    allow(
        dead_code,
        reason = "Runtime-only consumers cannot construct authoring gradients."
    )
)]
enum GradientKind {
    LinearGradient,
}

#[cfg(feature = "compile")]
patch!(CanvasPatch { textures: Vec<Texture>, padding_px: f64 });

/// A bounded canvas texture. An omitted texture color uses the theme's guide color.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(missing_docs)] // Variant fields use the JSON Schema names and units.
pub enum Texture {
    None,
    Grid {
        step_px: f64,
        major_every: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        opacity: f64,
        major_opacity: f64,
    },
    Dots {
        step_px: f64,
        radius_px: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        opacity: f64,
    },
    Ruled {
        line_height_px: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        opacity: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        margin_rule_color: Option<Color>,
        margin_rule_x_px: f64,
    },
    Fibers {
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        opacity: f64,
        angle_deg: f64,
        cross: bool,
    },
    Scanlines {
        step_px: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<Color>,
        opacity: f64,
    },
    RadialWash {
        color: Color,
        opacity: f64,
        cx: f64,
        cy: f64,
        radius: f64,
    },
}

#[cfg(feature = "compile")]
#[derive(Deserialize)]
#[serde(variant_identifier, rename_all = "snake_case")]
enum TextureKind {
    None,
    Grid,
    Dots,
    Ruled,
    Fibers,
    Scanlines,
    RadialWash,
}

#[cfg(feature = "compile")]
tagged_fields!(TextureFields {
    kind: TextureKind,
    step_px: f64,
    major_every: u8,
    color: Color,
    opacity: f64,
    major_opacity: f64,
    radius_px: f64,
    line_height_px: f64,
    margin_rule_color: Color,
    margin_rule_x_px: f64,
    angle_deg: f64,
    cross: bool,
    cx: f64,
    cy: f64,
    radius: f64,
});

#[cfg(feature = "compile")]
impl<'de> Deserialize<'de> for Texture {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TextureVisitor;

        impl<'de> de::Visitor<'de> for TextureVisitor {
            type Value = Texture;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canvas texture object")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Texture, A::Error> {
                const DEFAULT_OPACITY: f64 = 0.1;

                let mut fields = TextureFields::deserialize(MapAccessDeserializer::new(map))?;
                let texture = match fields.kind {
                    TextureKind::None => Texture::None,
                    TextureKind::Grid => Texture::Grid {
                        step_px: take_required::<_, A::Error>(&mut fields.step_px, "step_px")?,
                        major_every: fields.major_every.take().unwrap_or(5),
                        color: fields.color.take(),
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                        major_opacity: fields.major_opacity.take().unwrap_or(0.2),
                    },
                    TextureKind::Dots => Texture::Dots {
                        step_px: take_required::<_, A::Error>(&mut fields.step_px, "step_px")?,
                        radius_px: fields.radius_px.take().unwrap_or(1.0),
                        color: fields.color.take(),
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                    },
                    TextureKind::Ruled => Texture::Ruled {
                        line_height_px: take_required::<_, A::Error>(
                            &mut fields.line_height_px,
                            "line_height_px",
                        )?,
                        color: fields.color.take(),
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                        margin_rule_color: fields.margin_rule_color.take(),
                        margin_rule_x_px: fields.margin_rule_x_px.take().unwrap_or(32.0),
                    },
                    TextureKind::Fibers => Texture::Fibers {
                        color: fields.color.take(),
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                        angle_deg: fields.angle_deg.take().unwrap_or(120.0),
                        cross: fields.cross.take().unwrap_or_default(),
                    },
                    TextureKind::Scanlines => Texture::Scanlines {
                        step_px: take_required::<_, A::Error>(&mut fields.step_px, "step_px")?,
                        color: fields.color.take(),
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                    },
                    TextureKind::RadialWash => Texture::RadialWash {
                        color: take_required::<_, A::Error>(&mut fields.color, "color")?,
                        opacity: fields.opacity.take().unwrap_or(DEFAULT_OPACITY),
                        cx: fields.cx.take().unwrap_or(0.5),
                        cy: fields.cy.take().unwrap_or(0.5),
                        radius: fields.radius.take().unwrap_or(1.0),
                    },
                };
                fields.reject_unused::<A::Error>()?;
                Ok(texture)
            }
        }

        deserializer.deserialize_map(TextureVisitor)
    }
}

#[cfg(feature = "compile")]
fn opaque() -> f64 {
    1.0
}

/// A colored shadow at a bounded pixel offset and blur radius.
#[cfg_attr(feature = "compile", derive(Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "compile", serde(deny_unknown_fields))]
#[non_exhaustive]
pub struct ShadowLayer {
    /// Horizontal pixel offset.
    pub dx: f64,
    /// Vertical pixel offset.
    pub dy: f64,
    /// Gaussian blur radius in pixels. Zero is a hard offset.
    #[cfg_attr(feature = "compile", serde(default))]
    pub blur: f64,
    /// Shadow color.
    pub color: Color,
    /// Shadow opacity.
    #[cfg_attr(feature = "compile", serde(default = "opaque"))]
    pub opacity: f64,
}

impl ShadowLayer {
    /// Describes one resolved shadow in the renderer's coordinate system.
    pub const fn new(dx: f64, dy: f64, blur: f64, color: Color, opacity: f64) -> Self {
        Self {
            dx,
            dy,
            blur,
            color,
            opacity,
        }
    }
}

#[cfg(feature = "compile")]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ElevationSpec {
    Preset(ElevationPreset),
    Effect(ElevationEffect),
}

#[cfg(feature = "compile")]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ElevationPreset {
    Flat,
    Soft,
    Crisp,
}

#[cfg(feature = "compile")]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ElevationEffect {
    LayeredStack {
        layers: Vec<ShadowLayer>,
    },
    Glow {
        color: Color,
        blur: f64,
        opacity: f64,
    },
}

#[cfg(feature = "compile")]
#[derive(Deserialize)]
#[serde(variant_identifier, rename_all = "snake_case")]
enum ElevationEffectKind {
    LayeredStack,
    Glow,
}

#[cfg(feature = "compile")]
tagged_fields!(ElevationFields {
    kind: ElevationEffectKind,
    layers: Vec<super::json::Object<ShadowLayer>>,
    color: Color,
    blur: f64,
    opacity: f64,
});

#[cfg(feature = "compile")]
impl<'de> Deserialize<'de> for ElevationSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ElevationVisitor;

        impl<'de> de::Visitor<'de> for ElevationVisitor {
            type Value = ElevationSpec;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an elevation preset string or an elevation effect object")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<ElevationSpec, E> {
                ElevationPreset::deserialize(value.into_deserializer()).map(ElevationSpec::Preset)
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<ElevationSpec, A::Error> {
                let mut fields = ElevationFields::deserialize(MapAccessDeserializer::new(map))?;
                let effect = match fields.kind {
                    ElevationEffectKind::LayeredStack => ElevationEffect::LayeredStack {
                        layers: take_required::<_, A::Error>(&mut fields.layers, "layers")?
                            .into_iter()
                            .map(super::json::Object::into_inner)
                            .collect(),
                    },
                    ElevationEffectKind::Glow => ElevationEffect::Glow {
                        color: take_required::<_, A::Error>(&mut fields.color, "color")?,
                        blur: take_required::<_, A::Error>(&mut fields.blur, "blur")?,
                        opacity: fields.opacity.take().unwrap_or(0.35),
                    },
                };
                fields.reject_unused::<A::Error>()?;
                Ok(ElevationSpec::Effect(effect))
            }
        }

        deserializer.deserialize_any(ElevationVisitor)
    }
}

/// A typography slot for body, title, or code.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontRole {
    /// Use the sans-serif stack.
    Sans,
    /// Use the serif stack.
    Serif,
    /// Use the monospace stack.
    Mono,
}

/// Concrete font stacks, sizes, and weights.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Typography {
    /// Ordered sans-serif font families.
    pub sans: Vec<String>,
    /// Ordered monospace font families.
    pub mono: Vec<String>,
    /// Ordered serif font families.
    pub serif: Vec<String>,
    /// Body typography slot.
    pub body_family: FontRole,
    /// Title typography slot.
    pub title_family: FontRole,
    /// Code typography slot.
    pub code_family: FontRole,
    /// Base text size in CSS pixels.
    pub base_size_px: f64,
    /// CSS weight used for titles.
    pub title_weight: u16,
    /// CSS weight used for labels.
    pub label_weight: u16,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            sans: vec!["Arial".into(), "sans-serif".into()],
            mono: vec!["Consolas".into(), "monospace".into()],
            serif: vec!["Georgia".into(), "serif".into()],
            body_family: FontRole::Sans,
            title_family: FontRole::Sans,
            code_family: FontRole::Mono,
            base_size_px: 14.0,
            title_weight: 600,
            label_weight: 500,
        }
    }
}

impl Typography {
    /// Returns the font names in the requested slot, in fallback order.
    pub fn stack(&self, role: FontRole) -> &[String] {
        match role {
            FontRole::Sans => &self.sans,
            FontRole::Serif => &self.serif,
            FontRole::Mono => &self.mono,
        }
    }

    #[cfg(feature = "compile")]
    pub(super) fn apply(&mut self, patch: &TypographyPatch) {
        overlay!(
            self,
            patch,
            sans,
            mono,
            serif,
            body_family,
            title_family,
            code_family,
            base_size_px,
            title_weight,
            label_weight
        );
    }
}

#[cfg(feature = "compile")]
patch!(TypographyPatch {
    sans: Vec<String>, mono: Vec<String>, serif: Vec<String>,
    body_family: FontRole, title_family: FontRole, code_family: FontRole,
    base_size_px: f64, title_weight: u16, label_weight: u16,
});

/// The spacing and padding preference for layout.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Tight, space-efficient layout.
    Compact,
    /// Standard spacing.
    Balanced,
    /// More space between elements.
    Airy,
}

/// The preferred connector geometry.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeStyle {
    /// Curved connectors.
    Curved,
    /// Rectilinear connectors.
    Orthogonal,
    /// Straight segments.
    Straight,
    /// Sketch-style rendering where supported.
    Sketch,
}

/// The preferred arrowhead shape for ordinary connectors.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerStyle {
    /// A narrow arrowhead with a recessed base.
    Stealth,
    /// A filled triangle.
    Filled,
    /// An open chevron.
    Open,
    /// Use the family's classic shape.
    Classic,
}

/// How a label on a connector should be backed.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeLabelBackground {
    /// Use the underlying container or canvas.
    Auto,
    /// Use the canvas paint.
    Canvas,
    /// Use a bordered node-colored capsule.
    Pill,
    /// Draw only the label.
    Transparent,
}

/// Concrete chart presentation settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct ChartPolish {
    /// Draw reference gridlines.
    pub gridlines: bool,
    /// Radius of the outer ends of bars, in pixels.
    pub bar_radius: f64,
    /// Use a fine outline and translucent fill for bars.
    pub bar_wireframe: bool,
    /// Show data point markers on line charts.
    pub line_markers: bool,
    /// Separator width between pie slices, in pixels.
    pub pie_gap_px: f64,
    /// Opacity of pie fills.
    pub pie_opacity: f64,
}

impl Default for ChartPolish {
    fn default() -> Self {
        Self {
            gridlines: false,
            bar_radius: 0.0,
            bar_wireframe: false,
            line_markers: false,
            pie_gap_px: 0.0,
            pie_opacity: 0.92,
        }
    }
}

#[cfg(feature = "compile")]
patch!(ChartPatch {
    gridlines: bool,
    bar_radius: f64,
    bar_wireframe: bool,
    line_markers: bool,
    pie_gap_px: f64,
    pie_opacity: f64,
});

/// Concrete shape, connector, and chart presentation settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Form {
    /// Preferred layout density.
    pub density: Density,
    /// Node radius in pixels.
    pub radius: f64,
    /// Container radius in pixels.
    pub container_radius: f64,
    /// Shape outline width in pixels.
    pub stroke_width: f64,
    /// Connector width in pixels.
    pub connector_width: f64,
    /// Preferred connector geometry.
    pub edge_style: EdgeStyle,
    /// Preferred arrowhead shape.
    pub marker_style: MarkerStyle,
    /// Preferred connector label background.
    pub edge_label_bg: EdgeLabelBackground,
    /// Chart presentation settings.
    pub chart: ChartPolish,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            density: Density::Balanced,
            radius: 4.0,
            container_radius: 6.0,
            stroke_width: 1.0,
            connector_width: 1.5,
            edge_style: EdgeStyle::Curved,
            marker_style: MarkerStyle::Classic,
            edge_label_bg: EdgeLabelBackground::Auto,
            chart: ChartPolish::default(),
        }
    }
}

#[cfg(feature = "compile")]
impl Form {
    pub(super) fn apply(&mut self, patch: &FormPatch) {
        overlay!(
            self,
            patch,
            density,
            radius,
            container_radius,
            stroke_width,
            connector_width,
            edge_style,
            marker_style,
            edge_label_bg
        );
        if let Some(chart) = &patch.chart {
            overlay!(
                self.chart,
                chart,
                gridlines,
                bar_radius,
                bar_wireframe,
                line_markers,
                pie_gap_px,
                pie_opacity
            );
        }
    }
}

#[cfg(feature = "compile")]
patch!(FormPatch {
    density: Density,
    radius: f64,
    container_radius: f64,
    stroke_width: f64,
    connector_width: f64,
    edge_style: EdgeStyle,
    marker_style: MarkerStyle,
    edge_label_bg: EdgeLabelBackground,
    elevation: ElevationSpec,
} objects {
    chart: ChartPatch,
});

#[cfg(feature = "compile")]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FamilyOverride {
    pub(super) families: Vec<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) appearance: Option<Appearance>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) roles: Option<RoleOverrides>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) form: Option<FormPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) elevation: Option<ElevationSpec>,
    #[serde(default, deserialize_with = "present_object")]
    pub(super) typography: Option<TypographyPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) node_ordinal_palette: Option<bool>,
}
