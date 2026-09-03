use crate::{Color, DrawingListError, Point};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

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
    Image(ImageResource),
}

impl DrawingResource {
    pub fn id(&self) -> &ResourceId {
        match self {
            Self::Path(resource) => &resource.id,
            Self::LinearGradient(resource) => &resource.id,
            Self::Image(resource) => &resource.id,
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
    ) -> Result<(), DrawingListError> {
        match self {
            Self::Path(resource) => resource.validate(max_path_segments),
            Self::LinearGradient(resource) => resource.validate(),
            Self::Image(resource) => resource.validate(max_image_bytes),
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
    pub stops: Vec<GradientStop>,
}

impl LinearGradientResource {
    fn validate(&self) -> Result<(), DrawingListError> {
        if self.id.as_str().is_empty() || !self.start.is_finite() || !self.end.is_finite() {
            return Err(DrawingListError::invalid(
                "linear gradient has invalid identity or geometry",
            ));
        }
        if self.stops.is_empty() {
            return Err(DrawingListError::invalid(format!(
                "linear gradient {} must contain stops",
                self.id.as_str()
            )));
        }
        let mut previous = -f64::EPSILON;
        for stop in &self.stops {
            if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
                return Err(DrawingListError::invalid(format!(
                    "linear gradient {} has an invalid stop offset",
                    self.id.as_str()
                )));
            }
            if stop.offset < previous {
                return Err(DrawingListError::invalid(format!(
                    "linear gradient {} stops must be ordered",
                    self.id.as_str()
                )));
            }
            previous = stop.offset;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodedImage {
    pub media_type: String,
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

impl EncodedImage {
    pub fn new(media_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            media_type: media_type.into(),
            data,
        }
    }
}

impl fmt::Display for EncodedImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.media_type)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageResource {
    pub id: ResourceId,
    pub image: EncodedImage,
    pub pixel_width: u32,
    pub pixel_height: u32,
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
        if !self.image.media_type.starts_with("image/") {
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
