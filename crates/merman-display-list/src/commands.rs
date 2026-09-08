use crate::{DrawingListError, Point, Rect, ResourceId, Transform};
use serde::{Deserialize, Serialize, de};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingListPolicy {
    AllowRasterSubtree,
    VectorOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrokeStyle {
    pub paint: crate::Paint,
    pub width: f64,
    pub dash_array: Vec<f64>,
    pub dash_offset: f64,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f64,
}

impl StrokeStyle {
    pub(crate) fn validate(&self) -> Result<(), DrawingListError> {
        if !self.width.is_finite()
            || self.width < 0.0
            || !self.miter_limit.is_finite()
            || self.miter_limit <= 0.0
            || !self.dash_offset.is_finite()
            || self
                .dash_array
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(DrawingListError::invalid(
                "stroke style contains invalid numeric values",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathStyle {
    pub fill_rule: FillRule,
    /// `None` disables filling. With no stroke either, geometry and semantics remain present
    /// but the command produces no paint (for example an unpainted SVG interaction shape).
    pub fill: Option<crate::Paint>,
    pub stroke: Option<StrokeStyle>,
}

impl PathStyle {
    pub(crate) fn validate(&self) -> Result<(), DrawingListError> {
        if let Some(stroke) = &self.stroke {
            stroke.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DrawingCommand {
    Save,
    Restore,
    SetOpacity {
        opacity: f64,
    },
    SetBlendMode {
        blend_mode: BlendMode,
    },
    ConcatTransform {
        transform: Transform,
    },
    BeginLayer {
        bounds: Rect,
        opacity: f64,
        blend_mode: BlendMode,
    },
    EndLayer,
    DrawPath {
        path: ResourceId,
        style: PathStyle,
    },
    ClipPath {
        path: ResourceId,
        fill_rule: FillRule,
    },
    DrawImage {
        image: ResourceId,
        bounds: Rect,
        opacity: f64,
    },
    BeginSemanticGroup {
        semantic_id: String,
    },
    EndSemanticGroup,
    DrawText {
        run: Box<TextRun>,
    },
    DrawRasterSubtree {
        fallback_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DrawingCommandWire {
    Save {},
    Restore {},
    SetOpacity {
        opacity: f64,
    },
    SetBlendMode {
        blend_mode: BlendMode,
    },
    ConcatTransform {
        transform: Transform,
    },
    BeginLayer {
        bounds: Rect,
        opacity: f64,
        blend_mode: BlendMode,
    },
    EndLayer {},
    DrawPath {
        path: ResourceId,
        style: PathStyle,
    },
    ClipPath {
        path: ResourceId,
        fill_rule: FillRule,
    },
    DrawImage {
        image: ResourceId,
        bounds: Rect,
        opacity: f64,
    },
    BeginSemanticGroup {
        semantic_id: String,
    },
    EndSemanticGroup {},
    DrawText {
        run: Box<TextRun>,
    },
    DrawRasterSubtree {
        fallback_id: String,
    },
}

impl<'de> Deserialize<'de> for DrawingCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let command = DrawingCommandWire::deserialize(deserializer)?;
        Ok(match command {
            DrawingCommandWire::Save {} => Self::Save,
            DrawingCommandWire::Restore {} => Self::Restore,
            DrawingCommandWire::SetOpacity { opacity } => Self::SetOpacity { opacity },
            DrawingCommandWire::SetBlendMode { blend_mode } => Self::SetBlendMode { blend_mode },
            DrawingCommandWire::ConcatTransform { transform } => {
                Self::ConcatTransform { transform }
            }
            DrawingCommandWire::BeginLayer {
                bounds,
                opacity,
                blend_mode,
            } => Self::BeginLayer {
                bounds,
                opacity,
                blend_mode,
            },
            DrawingCommandWire::EndLayer {} => Self::EndLayer,
            DrawingCommandWire::DrawPath { path, style } => Self::DrawPath { path, style },
            DrawingCommandWire::ClipPath { path, fill_rule } => Self::ClipPath { path, fill_rule },
            DrawingCommandWire::DrawImage {
                image,
                bounds,
                opacity,
            } => Self::DrawImage {
                image,
                bounds,
                opacity,
            },
            DrawingCommandWire::BeginSemanticGroup { semantic_id } => {
                Self::BeginSemanticGroup { semantic_id }
            }
            DrawingCommandWire::EndSemanticGroup {} => Self::EndSemanticGroup,
            DrawingCommandWire::DrawText { run } => Self::DrawText { run },
            DrawingCommandWire::DrawRasterSubtree { fallback_id } => {
                Self::DrawRasterSubtree { fallback_id }
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRun {
    pub text: String,
    pub origin: Point,
    pub bounds: Rect,
    pub style: TextStyle,
    pub anchor: TextAnchor,
    pub baseline: TextBaseline,
    pub direction: TextDirection,
    pub language: Option<String>,
    pub obligation: TextObligation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    pub font: FontDescriptor,
    pub font_size: f64,
    pub letter_spacing: f64,
    pub line_height: f64,
    pub fill: crate::Paint,
    pub stroke: Option<StrokeStyle>,
    pub paint_order: TextPaintOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextPaintOrder {
    /// Paint the fill first and the stroke on top, matching the SVG default.
    FillThenStroke,
    /// Paint the stroke first and the fill on top, matching CSS `paint-order: stroke`.
    StrokeThenFill,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontDescriptor {
    pub families: Vec<String>,
    pub weight: u16,
    pub style: FontStyle,
    pub postscript_name: Option<String>,
    pub resource: Option<ResourceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextBaseline {
    Alphabetic,
    Hanging,
    Ideographic,
    Middle,
    Central,
    TextBeforeEdge,
    TextAfterEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub origin: Point,
    pub advance: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextObligation {
    HostText { measurement: MeasurementProvenance },
    GlyphRun { glyphs: Vec<PositionedGlyph> },
    Outline { path: ResourceId },
    RasterFallback { fallback_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MeasurementProvenance {
    HostCallback { profile: String },
    DeterministicFallback { profile: String },
    GlyphGeometry,
}

impl DrawingCommand {
    /// Creates a command that draws the provided text run.
    pub fn draw_text(run: TextRun) -> Self {
        Self::DrawText { run: Box::new(run) }
    }

    pub(crate) fn validate_numbers(&self) -> Result<(), DrawingListError> {
        match self {
            Self::SetOpacity { opacity } => validate_unit(*opacity, "opacity"),
            Self::ConcatTransform { transform } => {
                if transform.is_finite() {
                    Ok(())
                } else {
                    Err(DrawingListError::invalid(
                        "transform contains non-finite values",
                    ))
                }
            }
            Self::BeginLayer {
                bounds, opacity, ..
            } => {
                if !bounds.is_valid() {
                    return Err(DrawingListError::invalid("layer bounds are invalid"));
                }
                validate_unit(*opacity, "layer opacity")
            }
            Self::DrawPath { style, .. } => style.validate(),
            Self::ClipPath { .. } => Ok(()),
            Self::DrawImage {
                bounds, opacity, ..
            } => {
                if !bounds.is_valid() {
                    return Err(DrawingListError::invalid("image bounds are invalid"));
                }
                validate_unit(*opacity, "image opacity")
            }
            Self::DrawText { run } => run.validate(),
            Self::Save
            | Self::Restore
            | Self::SetBlendMode { .. }
            | Self::EndLayer
            | Self::BeginSemanticGroup { .. }
            | Self::EndSemanticGroup
            | Self::DrawRasterSubtree { .. } => Ok(()),
        }
    }
}

impl TextRun {
    fn validate(&self) -> Result<(), DrawingListError> {
        if !self.origin.is_finite() || !self.bounds.is_valid() {
            return Err(DrawingListError::invalid("text run geometry is invalid"));
        }
        if self.language.as_deref().is_some_and(str::is_empty) {
            return Err(DrawingListError::invalid(
                "text language must not be empty when present",
            ));
        }
        self.style.validate()?;
        if self.style.font.families.is_empty()
            || self.style.font.families.iter().any(String::is_empty)
            || self.style.font.weight == 0
            || self
                .style
                .font
                .postscript_name
                .as_deref()
                .is_some_and(str::is_empty)
            || self
                .style
                .font
                .resource
                .as_ref()
                .is_some_and(|resource| resource.as_str().is_empty())
        {
            return Err(DrawingListError::invalid(
                "text font descriptor is incomplete",
            ));
        }
        match &self.obligation {
            TextObligation::HostText { measurement } => {
                if matches!(measurement, MeasurementProvenance::HostCallback { profile } if profile.is_empty())
                    || matches!(measurement, MeasurementProvenance::DeterministicFallback { profile } if profile.is_empty())
                {
                    return Err(DrawingListError::invalid(
                        "text measurement profile must not be empty",
                    ));
                }
            }
            TextObligation::GlyphRun { glyphs } => {
                if glyphs
                    .iter()
                    .any(|glyph| !glyph.origin.is_finite() || !glyph.advance.is_finite())
                {
                    return Err(DrawingListError::invalid(
                        "glyph run contains non-finite positioning",
                    ));
                }
            }
            TextObligation::Outline { .. } | TextObligation::RasterFallback { .. } => {}
        }
        Ok(())
    }
}

impl TextStyle {
    fn validate(&self) -> Result<(), DrawingListError> {
        if !self.font_size.is_finite()
            || self.font_size <= 0.0
            || !self.letter_spacing.is_finite()
            || !self.line_height.is_finite()
            || self.line_height <= 0.0
        {
            return Err(DrawingListError::invalid(
                "text style contains invalid numeric values",
            ));
        }
        if let Some(stroke) = &self.stroke {
            stroke.validate()?;
        }
        Ok(())
    }
}

fn validate_unit(value: f64, name: &str) -> Result<(), DrawingListError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(DrawingListError::invalid(format!(
            "{name} must be finite and between 0 and 1"
        )))
    }
}
