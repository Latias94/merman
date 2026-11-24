use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

use super::Color;

// Optional fields in the document are omitted, so JSON null cannot be mistaken for an override.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

macro_rules! patch {
    ($name:ident { $($(#[$attr:meta])* $field:ident : $kind:ty),+ $(,)? }) => {
        #[derive(Clone, Debug, Default, Deserialize, PartialEq)]
        #[serde(default, deny_unknown_fields)]
        pub(super) struct $name {
            $(
                $(#[$attr])*
                #[serde(deserialize_with = "present")]
                pub(super) $field: Option<$kind>,
            )+
        }
    };
}

macro_rules! overlay {
    ($target:expr, $patch:expr, $($field:ident),+ $(,)?) => {
        $(if let Some(value) = &$patch.$field {
            $target.$field.clone_from(value);
        })+
    };
}

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
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ThemeDocumentV2 {
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
    #[serde(default, deserialize_with = "present")]
    pub(super) seed: Option<SeedPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) roles: Option<RoleOverrides>,
    #[serde(default, deserialize_with = "present")]
    pub(super) typography: Option<TypographyPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) form: Option<FormPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) appearances: Option<Appearances>,
    #[serde(default)]
    pub(super) overrides: Vec<FamilyOverride>,
}

impl ThemeDocumentV2 {
    /// The document's own identifier, when supplied.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// The display name supplied by the author.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// A short description of this design.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// The design's catalog category.
    pub const fn category(&self) -> Option<Category> {
        self.category
    }

    /// The built-in document this theme extends, when supplied.
    pub fn extends(&self) -> Option<&str> {
        self.extends.as_deref()
    }

    /// Light and dark layers declared in this document, before preset extension.
    pub fn appearances(&self) -> impl Iterator<Item = Appearance> + '_ {
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

patch!(Appearances {
    light: AppearanceLayer,
    dark: AppearanceLayer
});
patch!(AppearanceLayer {
    seed: SeedPatch,
    roles: RoleOverrides,
    data: DataPatch,
    canvas: CanvasPatch,
    elevation: ElevationSpec,
});
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

        patch!(RoleOverrides { $(#[serde(rename = $key)] $field: $kind,)+ });

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

patch!(DataPatch {
    ink: Vec<Color>,
    fill: Vec<Color>,
    fill_ink: FillInk,
    pattern_encoding: bool,
});

impl DataPatch {
    pub(super) fn merge(&mut self, patch: &Self) {
        merge!(self, patch, ink, fill, fill_ink, pattern_encoding);
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(super) enum FillInk {
    Auto(Auto),
    Color(Color),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
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
    /// Returns the starting color for surfaces that use a single color.
    pub fn first_color(&self) -> Color {
        match self {
            Self::Solid(color) => *color,
            Self::LinearGradient(gradient) => gradient
                .stops
                .first()
                .map_or(Color::TRANSPARENT, |stop| stop.color),
        }
    }
}

impl<'de> Deserialize<'de> for Paint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Solid(String),
            LinearGradient(LinearGradient),
        }

        Ok(match Value::deserialize(deserializer)? {
            Value::Solid(value) if value == "transparent" => Self::Solid(Color::TRANSPARENT),
            Value::Solid(value) => Self::Solid(Color::parse(&value).map_err(de::Error::custom)?),
            Value::LinearGradient(gradient) => Self::LinearGradient(gradient),
        })
    }
}

/// A stop in a linear gradient.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct GradientStop {
    /// Gradient position between zero and one.
    pub offset: f64,
    /// Stop color.
    pub color: Color,
    /// Opacity applied in addition to the color's alpha.
    #[serde(default = "opaque")]
    pub opacity: f64,
}

/// A linear gradient along an axis specified in degrees.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinearGradient {
    kind: GradientKind,
    /// Angle in degrees.
    #[serde(default)]
    pub angle_deg: f64,
    /// Stops in nondecreasing offset order.
    pub stops: Vec<GradientStop>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum GradientKind {
    LinearGradient,
}

patch!(CanvasPatch { textures: Vec<Texture>, padding_px: f64 });

/// A bounded canvas texture. An omitted texture color uses the theme's guide color.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(missing_docs)] // Variant fields use the JSON Schema names and units.
pub enum Texture {
    None,
    Grid {
        step_px: f64,
        #[serde(default = "default_major_every")]
        major_every: u8,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        color: Option<Color>,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
        #[serde(default = "default_major_opacity")]
        major_opacity: f64,
    },
    Dots {
        step_px: f64,
        #[serde(default = "opaque")]
        radius_px: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        color: Option<Color>,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
    },
    Ruled {
        line_height_px: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        color: Option<Color>,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        margin_rule_color: Option<Color>,
        #[serde(default = "default_margin_rule_x")]
        margin_rule_x_px: f64,
    },
    Fibers {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        color: Option<Color>,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
        #[serde(default = "default_fiber_angle")]
        angle_deg: f64,
        #[serde(default)]
        cross: bool,
    },
    Scanlines {
        step_px: f64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present"
        )]
        color: Option<Color>,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
    },
    RadialWash {
        color: Color,
        #[serde(default = "default_texture_opacity")]
        opacity: f64,
        #[serde(default = "half")]
        cx: f64,
        #[serde(default = "half")]
        cy: f64,
        #[serde(default = "opaque")]
        radius: f64,
    },
}

fn default_major_every() -> u8 {
    5
}
fn default_texture_opacity() -> f64 {
    0.1
}
fn default_major_opacity() -> f64 {
    0.2
}
fn default_margin_rule_x() -> f64 {
    32.0
}
fn default_fiber_angle() -> f64 {
    120.0
}
fn half() -> f64 {
    0.5
}
fn opaque() -> f64 {
    1.0
}

/// A colored shadow at a bounded pixel offset and blur radius.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShadowLayer {
    /// Horizontal pixel offset.
    pub dx: f64,
    /// Vertical pixel offset.
    pub dy: f64,
    /// Gaussian blur radius in pixels. Zero is a hard offset.
    #[serde(default)]
    pub blur: f64,
    /// Shadow color.
    pub color: Color,
    /// Shadow opacity.
    #[serde(default = "opaque")]
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

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(super) enum ElevationSpec {
    Preset(ElevationPreset),
    Effect(ElevationEffect),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ElevationPreset {
    Flat,
    Soft,
    Crisp,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ElevationEffect {
    LayeredStack {
        layers: Vec<ShadowLayer>,
    },
    Glow {
        color: Color,
        blur: f64,
        #[serde(default = "default_glow_opacity")]
        opacity: f64,
    },
}

fn default_glow_opacity() -> f64 {
    0.35
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
    chart: ChartPatch,
});

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FamilyOverride {
    pub(super) families: Vec<String>,
    #[serde(default, deserialize_with = "present")]
    pub(super) appearance: Option<Appearance>,
    #[serde(default, deserialize_with = "present")]
    pub(super) roles: Option<RoleOverrides>,
    #[serde(default, deserialize_with = "present")]
    pub(super) form: Option<FormPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) elevation: Option<ElevationSpec>,
    #[serde(default, deserialize_with = "present")]
    pub(super) typography: Option<TypographyPatch>,
    #[serde(default, deserialize_with = "present")]
    pub(super) node_ordinal_palette: Option<bool>,
}
