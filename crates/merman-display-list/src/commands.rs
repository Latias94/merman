use crate::validation::{ValidationControl, ValidationEvent};
use crate::{DrawingListError, Point, Rect, ResourceId, Transform};
use serde::{Deserialize, Serialize, de, de::MapAccess, de::Visitor};
use std::fmt;

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
    pub(crate) fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if !self.width.is_finite()
            || self.width < 0.0
            || !self.miter_limit.is_finite()
            || self.miter_limit <= 0.0
            || !self.dash_offset.is_finite()
        {
            return Err(
                DrawingListError::invalid("stroke style contains invalid numeric values").into(),
            );
        }
        for value in &self.dash_array {
            control.checkpoint()?;
            if !value.is_finite() || *value < 0.0 {
                return Err(DrawingListError::invalid(
                    "stroke style contains invalid numeric values",
                )
                .into());
            }
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
    /// Both paint fields are required on the wire, even when their value is `null`.
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub fill: Option<crate::Paint>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub stroke: Option<StrokeStyle>,
}

impl PathStyle {
    pub(crate) fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if let Some(stroke) = &self.stroke {
            stroke.validate(control)?;
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
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub language: Option<String>,
    pub obligation: TextObligation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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

impl<'de> Deserialize<'de> for TextStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "font",
            "font_size",
            "letter_spacing",
            "line_height",
            "fill",
            "stroke",
            "paint_order",
        ];

        struct TextStyleVisitor;

        impl<'de> Visitor<'de> for TextStyleVisitor {
            type Value = TextStyle;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a DrawingList text style object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut font = None;
                let mut font_size = None;
                let mut letter_spacing = None;
                let mut line_height = None;
                let mut fill = None;
                let mut stroke = None;
                let mut paint_order = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "font" => set_once(&mut font, map.next_value()?, "font")?,
                        "font_size" => set_once(&mut font_size, map.next_value()?, "font_size")?,
                        "letter_spacing" => {
                            set_once(&mut letter_spacing, map.next_value()?, "letter_spacing")?
                        }
                        "line_height" => {
                            set_once(&mut line_height, map.next_value()?, "line_height")?
                        }
                        "fill" => set_once(&mut fill, map.next_value()?, "fill")?,
                        "stroke" => set_once(&mut stroke, map.next_value()?, "stroke")?,
                        "paint_order" => {
                            set_once(&mut paint_order, map.next_value()?, "paint_order")?
                        }
                        other => return Err(de::Error::unknown_field(other, FIELDS)),
                    }
                }

                Ok(TextStyle {
                    font: font.ok_or_else(|| de::Error::missing_field("font"))?,
                    font_size: font_size.ok_or_else(|| de::Error::missing_field("font_size"))?,
                    letter_spacing: letter_spacing
                        .ok_or_else(|| de::Error::missing_field("letter_spacing"))?,
                    line_height: line_height
                        .ok_or_else(|| de::Error::missing_field("line_height"))?,
                    fill: fill.ok_or_else(|| de::Error::missing_field("fill"))?,
                    stroke: stroke.ok_or_else(|| de::Error::missing_field("stroke"))?,
                    paint_order: paint_order
                        .ok_or_else(|| de::Error::missing_field("paint_order"))?,
                })
            }
        }

        fn set_once<T, E>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
        where
            E: de::Error,
        {
            if slot.replace(value).is_some() {
                return Err(de::Error::duplicate_field(field));
            }
            Ok(())
        }

        deserializer.deserialize_map(TextStyleVisitor)
    }
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
    #[serde(deserialize_with = "Deserialize::deserialize")]
    pub postscript_name: Option<String>,
    #[serde(deserialize_with = "Deserialize::deserialize")]
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
    HostCallback {
        profile: String,
    },
    DeterministicFallback {
        profile: String,
    },
    #[serde(deserialize_with = "crate::wire::deserialize_empty_object")]
    GlyphGeometry,
}

impl DrawingCommand {
    /// Creates a command that draws the provided text run.
    pub fn draw_text(run: TextRun) -> Self {
        Self::DrawText { run: Box::new(run) }
    }

    pub(crate) fn validate_numbers<E, F>(
        &self,
        control: &mut ValidationControl<E, F>,
    ) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        control.checkpoint()?;
        match self {
            Self::SetOpacity { opacity } => validate_unit(*opacity, "opacity").map_err(E::from),
            Self::ConcatTransform { transform } => {
                if transform.is_finite() {
                    Ok(())
                } else {
                    Err(DrawingListError::invalid("transform contains non-finite values").into())
                }
            }
            Self::BeginLayer {
                bounds, opacity, ..
            } => {
                if !bounds.is_valid() {
                    return Err(DrawingListError::invalid("layer bounds are invalid").into());
                }
                validate_unit(*opacity, "layer opacity").map_err(E::from)
            }
            Self::DrawPath { style, .. } => style.validate(control),
            Self::ClipPath { .. } => Ok(()),
            Self::DrawImage {
                bounds, opacity, ..
            } => {
                if !bounds.is_valid() {
                    return Err(DrawingListError::invalid("image bounds are invalid").into());
                }
                validate_unit(*opacity, "image opacity").map_err(E::from)
            }
            Self::DrawText { run } => run.validate(control),
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
    fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if !self.origin.is_finite() || !self.bounds.is_valid() {
            return Err(DrawingListError::invalid("text run geometry is invalid").into());
        }
        if self.language.as_deref().is_some_and(str::is_empty) {
            return Err(
                DrawingListError::invalid("text language must not be empty when present").into(),
            );
        }
        self.style.validate(control)?;
        if self.style.font.families.is_empty()
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
            return Err(DrawingListError::invalid("text font descriptor is incomplete").into());
        }
        for family in &self.style.font.families {
            control.checkpoint()?;
            if family.is_empty() {
                return Err(DrawingListError::invalid("text font descriptor is incomplete").into());
            }
        }
        match &self.obligation {
            TextObligation::HostText { measurement } => {
                if matches!(measurement, MeasurementProvenance::HostCallback { profile } if profile.is_empty())
                    || matches!(measurement, MeasurementProvenance::DeterministicFallback { profile } if profile.is_empty())
                {
                    return Err(DrawingListError::invalid(
                        "text measurement profile must not be empty",
                    )
                    .into());
                }
            }
            TextObligation::GlyphRun { glyphs } => {
                for glyph in glyphs {
                    control.checkpoint()?;
                    if !glyph.origin.is_finite() || !glyph.advance.is_finite() {
                        return Err(DrawingListError::invalid(
                            "glyph run contains non-finite positioning",
                        )
                        .into());
                    }
                }
            }
            TextObligation::Outline { .. } | TextObligation::RasterFallback { .. } => {}
        }
        Ok(())
    }
}

impl TextStyle {
    fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if !self.font_size.is_finite()
            || self.font_size <= 0.0
            || !self.letter_spacing.is_finite()
            || !self.line_height.is_finite()
            || self.line_height <= 0.0
        {
            return Err(
                DrawingListError::invalid("text style contains invalid numeric values").into(),
            );
        }
        if let Some(stroke) = &self.stroke {
            stroke.validate(control)?;
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
