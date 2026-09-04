use crate::{Color, DrawingListError, Point, Rect, Transform};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Compares a media type's type/subtype while ignoring ASCII case and optional parameters.
///
/// The original wire spelling is intentionally preserved; this helper is only for validation.
pub(crate) fn media_type_matches(
    media_type: &str,
    expected_type: &str,
    expected_subtype: &str,
) -> bool {
    let Some((kind, subtype)) = media_type.split(';').next().and_then(|value| {
        let (kind, subtype) = value.trim().split_once('/')?;
        let kind = kind.trim();
        let subtype = subtype.trim();
        (!kind.is_empty() && !subtype.is_empty()).then_some((kind, subtype))
    }) else {
        return false;
    };
    kind.eq_ignore_ascii_case(expected_type) && subtype.eq_ignore_ascii_case(expected_subtype)
}

pub(crate) fn is_image_media_type(media_type: &str) -> bool {
    let Some((kind, _subtype)) = media_type.split(';').next().and_then(|value| {
        let (kind, subtype) = value.trim().split_once('/')?;
        let kind = kind.trim();
        let subtype = subtype.trim();
        (!kind.is_empty() && !subtype.is_empty()).then_some((kind, subtype))
    }) else {
        return false;
    };
    kind.eq_ignore_ascii_case("image")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(String);

impl ResourceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ResourceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ResourceId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DrawingResource {
    Path(PathResource),
    LinearGradient(LinearGradientResource),
    RadialGradient(RadialGradientResource),
    Image(ImageResource),
    Pattern(PatternResource),
    Font(FontResource),
}

impl DrawingResource {
    pub fn id(&self) -> &ResourceId {
        match self {
            Self::Path(resource) => &resource.id,
            Self::LinearGradient(resource) => &resource.id,
            Self::RadialGradient(resource) => &resource.id,
            Self::Image(resource) => &resource.id,
            Self::Pattern(resource) => &resource.id,
            Self::Font(resource) => &resource.id,
        }
    }

    pub fn path_mut(&mut self) -> Option<&mut PathResource> {
        match self {
            Self::Path(resource) => Some(resource),
            _ => None,
        }
    }

    pub(crate) fn validate(
        &self,
        max_path_segments: usize,
        max_image_bytes: usize,
        max_font_bytes: usize,
    ) -> Result<(), DrawingListError> {
        match self {
            Self::Path(resource) => resource.validate(max_path_segments),
            Self::LinearGradient(resource) => resource.validate(),
            Self::RadialGradient(resource) => resource.validate(),
            Self::Image(resource) => resource.validate(max_image_bytes),
            Self::Pattern(resource) => resource.validate(),
            Self::Font(resource) => resource.validate(max_font_bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathResource {
    pub id: ResourceId,
    pub segments: Vec<PathSegment>,
}

impl PathResource {
    pub(crate) fn validate(&self, max_segments: usize) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty() {
            return Err(DrawingListError::invalid(
                "path resource id must not be empty",
            ));
        }
        if self.segments.is_empty() {
            return Err(DrawingListError::invalid(format!(
                "path resource {} must contain at least one segment",
                self.id.as_str()
            )));
        }
        if !matches!(self.segments.first(), Some(PathSegment::MoveTo { .. })) {
            return Err(DrawingListError::invalid(format!(
                "path resource {} must begin with move_to",
                self.id.as_str()
            )));
        }
        if self.segments.len() > max_segments {
            return Err(DrawingListError::ResourceLimit {
                resource: "path_segments",
                actual: self.segments.len(),
                maximum: max_segments,
            });
        }
        if self.segments.iter().any(|segment| !segment.is_finite()) {
            return Err(DrawingListError::invalid(format!(
                "path resource {} contains non-finite geometry",
                self.id.as_str()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathSegment {
    MoveTo {
        to: Point,
    },
    LineTo {
        to: Point,
    },
    QuadTo {
        control: Point,
        to: Point,
    },
    CubicTo {
        control1: Point,
        control2: Point,
        to: Point,
    },
    ArcTo {
        radius_x: f64,
        radius_y: f64,
        x_axis_rotation_degrees: f64,
        large_arc: bool,
        sweep_clockwise: bool,
        to: Point,
    },
    Close,
}

impl PathSegment {
    fn is_finite(&self) -> bool {
        match self {
            Self::MoveTo { to } | Self::LineTo { to } => to.is_finite(),
            Self::QuadTo { control, to } => control.is_finite() && to.is_finite(),
            Self::CubicTo {
                control1,
                control2,
                to,
            } => control1.is_finite() && control2.is_finite() && to.is_finite(),
            Self::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                to,
                ..
            } => {
                radius_x.is_finite()
                    && *radius_x >= 0.0
                    && radius_y.is_finite()
                    && *radius_y >= 0.0
                    && x_axis_rotation_degrees.is_finite()
                    && to.is_finite()
            }
            Self::Close => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientSpread {
    Pad,
    Repeat,
    Reflect,
}

impl GradientStop {
    pub const fn new(offset: f64, color: Color) -> Self {
        Self { offset, color }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearGradientResource {
    pub id: ResourceId,
    pub start: Point,
    pub end: Point,
    pub transform: Transform,
    pub spread: GradientSpread,
    pub stops: Vec<GradientStop>,
}

impl LinearGradientResource {
    fn validate(&self) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty()
            || !self.start.is_finite()
            || !self.end.is_finite()
            || !self.transform.is_finite()
        {
            return Err(DrawingListError::invalid(
                "linear gradient has invalid identity or geometry",
            ));
        }
        validate_gradient_stops("linear gradient", &self.id, &self.stops)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadialGradientResource {
    pub id: ResourceId,
    pub center: Point,
    pub focal: Point,
    pub radius: f64,
    pub transform: Transform,
    pub spread: GradientSpread,
    pub stops: Vec<GradientStop>,
}

impl RadialGradientResource {
    fn validate(&self) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty()
            || !self.center.is_finite()
            || !self.focal.is_finite()
            || !self.radius.is_finite()
            || self.radius <= 0.0
            || !self.transform.is_finite()
        {
            return Err(DrawingListError::invalid(
                "radial gradient has invalid identity or geometry",
            ));
        }
        validate_gradient_stops("radial gradient", &self.id, &self.stops)
    }
}

fn validate_gradient_stops(
    kind: &str,
    id: &ResourceId,
    stops: &[GradientStop],
) -> Result<(), DrawingListError> {
    if stops.is_empty() {
        return Err(DrawingListError::invalid(format!(
            "{kind} {} must contain stops",
            id.as_str()
        )));
    }
    let mut previous = -f64::EPSILON;
    for stop in stops {
        if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
            return Err(DrawingListError::invalid(format!(
                "{kind} {} has an invalid stop offset",
                id.as_str()
            )));
        }
        if stop.offset < previous {
            return Err(DrawingListError::invalid(format!(
                "{kind} {} stops must be ordered",
                id.as_str()
            )));
        }
        previous = stop.offset;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodedAsset {
    pub media_type: String,
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

impl EncodedAsset {
    pub fn new(media_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            media_type: media_type.into(),
            data,
        }
    }
}

impl fmt::Display for EncodedAsset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.media_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageResource {
    pub id: ResourceId,
    pub image: EncodedAsset,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub has_alpha: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternRepeat {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternResource {
    pub id: ResourceId,
    pub image: ResourceId,
    pub tile: Rect,
    pub transform: Transform,
    pub repetition: PatternRepeat,
}

impl PatternResource {
    fn validate(&self) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty()
            || self.image.as_str().is_empty()
            || !self.tile.is_valid()
            || self.tile.width <= 0.0
            || self.tile.height <= 0.0
            || !self.transform.is_finite()
        {
            return Err(DrawingListError::invalid(
                "pattern has invalid identity or geometry",
            ));
        }
        Ok(())
    }
}

/// Backward-compatible name for callers that describe this resource as an image pattern.
pub type ImagePatternResource = PatternResource;

/// The generic encoded payload used by image and font resources.
pub type EncodedImage = EncodedAsset;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontResource {
    pub id: ResourceId,
    pub font: EncodedAsset,
}

impl FontResource {
    fn validate(&self, max_font_bytes: usize) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty() || self.font.data.is_empty() {
            return Err(DrawingListError::invalid(format!(
                "font resource {} has invalid metadata",
                self.id.as_str()
            )));
        }
        if !is_font_media_type(&self.font.media_type) {
            return Err(DrawingListError::invalid(format!(
                "font resource {} must use a supported font media type",
                self.id.as_str()
            )));
        }
        if self.font.data.len() > max_font_bytes {
            return Err(DrawingListError::ResourceLimit {
                resource: "font_bytes",
                actual: self.font.data.len(),
                maximum: max_font_bytes,
            });
        }
        Ok(())
    }
}

fn is_font_media_type(media_type: &str) -> bool {
    [
        ("font", "ttf"),
        ("font", "otf"),
        ("font", "woff"),
        ("font", "woff2"),
        ("application", "font-sfnt"),
        ("application", "vnd.ms-fontobject"),
    ]
    .into_iter()
    .any(|(kind, subtype)| media_type_matches(media_type, kind, subtype))
}

impl ImageResource {
    fn validate(&self, max_image_bytes: usize) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty()
            || self.image.media_type.is_empty()
            || self.image.data.is_empty()
            || self.pixel_width == 0
            || self.pixel_height == 0
        {
            return Err(DrawingListError::invalid(format!(
                "image resource {} has invalid metadata",
                self.id.as_str()
            )));
        }
        if !is_image_media_type(&self.image.media_type) {
            return Err(DrawingListError::invalid(format!(
                "image resource {} must use an image media type",
                self.id.as_str()
            )));
        }
        if self.image.data.len() > max_image_bytes {
            return Err(DrawingListError::ResourceLimit {
                resource: "image_bytes",
                actual: self.image.data.len(),
                maximum: max_image_bytes,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Paint {
    Solid { color: Color },
    Resource { id: ResourceId },
}

impl Paint {
    pub fn solid(color: Color) -> Self {
        Self::Solid { color }
    }

    pub fn resource(id: ResourceId) -> Self {
        Self::Resource { id }
    }
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
}
