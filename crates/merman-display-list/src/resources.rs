use crate::validation::{ValidationControl, ValidationEvent};
use crate::{Color, DrawingListError, Point, Rect, Transform};
use base64::{Engine as _, display::Base64Display, engine::general_purpose::STANDARD};
use png::ColorType;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};
use std::{fmt, io::Cursor};

/// Compares a media type's type/subtype while ignoring ASCII case and optional parameters.
///
/// The original wire spelling is intentionally preserved; this helper is only for validation.
pub(crate) fn media_type_matches(
    media_type: &str,
    expected_type: &str,
    expected_subtype: &str,
) -> bool {
    let value = trim_ascii_mime_whitespace(media_type);
    let type_and_subtype = match value.split_once(';') {
        Some((value, _parameters)) => trim_ascii_horizontal_whitespace(value),
        None => value,
    };
    let Some((kind, subtype)) = type_and_subtype
        .split_once('/')
        .and_then(|(kind, subtype)| {
            (!kind.is_empty()
                && !subtype.is_empty()
                && trim_ascii_mime_whitespace(kind) == kind
                && trim_ascii_mime_whitespace(subtype) == subtype)
                .then_some((kind, subtype))
        })
    else {
        return false;
    };
    kind.eq_ignore_ascii_case(expected_type) && subtype.eq_ignore_ascii_case(expected_subtype)
}

fn trim_ascii_mime_whitespace(value: &str) -> &str {
    value.trim_matches(|character| matches!(character, '\t' | '\n' | '\r' | ' '))
}

fn trim_ascii_horizontal_whitespace(value: &str) -> &str {
    value.trim_matches(|character| matches!(character, '\t' | ' '))
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

    pub(crate) fn validate<E, F>(
        &self,
        image_bytes_used: usize,
        image_pixels_used: usize,
        control: &mut ValidationControl<E, F>,
    ) -> Result<ResourceValidationUsage, E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        control.checkpoint()?;
        match self {
            Self::Path(resource) => resource.validate(control)?,
            Self::LinearGradient(resource) => resource.validate(control)?,
            Self::RadialGradient(resource) => resource.validate(control)?,
            Self::Image(resource) => {
                return resource.validate(image_bytes_used, image_pixels_used, control);
            }
            Self::Pattern(resource) => resource.validate()?,
            Self::Font(resource) => resource.validate(control)?,
        }
        Ok(ResourceValidationUsage::default())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ResourceValidationUsage {
    pub(crate) image_bytes: usize,
    pub(crate) image_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathResource {
    pub id: ResourceId,
    pub segments: Vec<PathSegment>,
}

impl PathResource {
    pub(crate) fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if self.id.as_str().is_empty() {
            return Err(DrawingListError::invalid("path resource id must not be empty").into());
        }
        if self.segments.is_empty() {
            return Err(DrawingListError::invalid(format!(
                "path resource {} must contain at least one segment",
                self.id.as_str()
            ))
            .into());
        }
        if !matches!(self.segments.first(), Some(PathSegment::MoveTo { .. })) {
            return Err(DrawingListError::invalid(format!(
                "path resource {} must begin with move_to",
                self.id.as_str()
            ))
            .into());
        }
        control.count("path_segments", self.segments.len() as u64)?;
        for segment in &self.segments {
            control.checkpoint()?;
            if !segment.is_finite() {
                return Err(DrawingListError::invalid(format!(
                    "path resource {} contains non-finite geometry",
                    self.id.as_str()
                ))
                .into());
            }
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
    #[serde(deserialize_with = "crate::wire::deserialize_empty_object")]
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
    fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if self.id.as_str().is_empty()
            || !self.start.is_finite()
            || !self.end.is_finite()
            || !self.transform.is_finite()
        {
            return Err(DrawingListError::invalid(
                "linear gradient has invalid identity or geometry",
            )
            .into());
        }
        validate_gradient_stops("linear gradient", &self.id, &self.stops, control)
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
    fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if self.id.as_str().is_empty()
            || !self.center.is_finite()
            || !self.focal.is_finite()
            || !self.radius.is_finite()
            || self.radius <= 0.0
            || !self.transform.is_finite()
        {
            return Err(DrawingListError::invalid(
                "radial gradient has invalid identity or geometry",
            )
            .into());
        }
        validate_gradient_stops("radial gradient", &self.id, &self.stops, control)
    }
}

fn validate_gradient_stops<E, F>(
    kind: &str,
    id: &ResourceId,
    stops: &[GradientStop],
    control: &mut ValidationControl<E, F>,
) -> Result<(), E>
where
    E: From<DrawingListError>,
    F: FnMut(ValidationEvent) -> Result<(), E>,
{
    if stops.is_empty() {
        return Err(DrawingListError::invalid(format!(
            "{kind} {} must contain stops",
            id.as_str()
        ))
        .into());
    }
    let mut previous = -f64::EPSILON;
    for stop in stops {
        control.checkpoint()?;
        if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
            return Err(DrawingListError::invalid(format!(
                "{kind} {} has an invalid stop offset",
                id.as_str()
            ))
            .into());
        }
        if stop.offset < previous {
            return Err(DrawingListError::invalid(format!(
                "{kind} {} stops must be ordered",
                id.as_str()
            ))
            .into());
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
    fn validate<E, F>(&self, control: &mut ValidationControl<E, F>) -> Result<(), E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if self.id.as_str().is_empty() || self.font.data.is_empty() {
            return Err(DrawingListError::invalid(format!(
                "font resource {} has invalid metadata",
                self.id.as_str()
            ))
            .into());
        }
        if !is_font_media_type(&self.font.media_type) {
            return Err(DrawingListError::invalid(format!(
                "font resource {} must use a supported font media type",
                self.id.as_str()
            ))
            .into());
        }
        control.count("font_bytes", self.font.data.len() as u64)?;
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
    fn validate<E, F>(
        &self,
        image_bytes_used: usize,
        image_pixels_used: usize,
        control: &mut ValidationControl<E, F>,
    ) -> Result<ResourceValidationUsage, E>
    where
        E: From<DrawingListError>,
        F: FnMut(ValidationEvent) -> Result<(), E>,
    {
        if self.id.as_str().is_empty()
            || self.image.media_type.is_empty()
            || self.image.data.is_empty()
            || self.pixel_width == 0
            || self.pixel_height == 0
        {
            return Err(DrawingListError::invalid(format!(
                "image resource {} has invalid metadata",
                self.id.as_str()
            ))
            .into());
        }
        let cumulative_image_bytes = image_bytes_used
            .checked_add(self.image.data.len())
            .ok_or_else(|| DrawingListError::invalid("image byte count overflows usize"))?;
        control.count("image_bytes", cumulative_image_bytes as u64)?;

        if !media_type_matches(&self.image.media_type, "image", "png") {
            return Err(DrawingListError::invalid(format!(
                "image resource {} must use static PNG",
                self.id.as_str()
            ))
            .into());
        }
        let mut cursor = Cursor::new(self.image.data.as_slice());
        let mut decoder = png::Decoder::new(&mut cursor);
        decoder.set_ignore_text_chunk(true);
        decoder.set_ignore_iccp_chunk(true);
        let (header_width, header_height) = {
            let header = decoder.read_header_info().map_err(|error| {
                DrawingListError::invalid(format!(
                    "image resource {} has an invalid PNG header: {error}",
                    self.id.as_str()
                ))
            })?;
            (header.width, header.height)
        };
        let header_pixels = u64::from(header_width) * u64::from(header_height);
        let cumulative_image_pixels = (image_pixels_used as u64).checked_add(header_pixels);
        control.count("image_pixels", cumulative_image_pixels.unwrap_or(u64::MAX))?;
        cumulative_image_pixels
            .ok_or_else(|| DrawingListError::invalid("image pixel count overflows u64"))?;
        let header_pixels = usize::try_from(header_pixels).map_err(|_| {
            DrawingListError::invalid("accepted image pixel count does not fit usize")
        })?;
        let mut reader = decoder.read_info().map_err(|error| {
            DrawingListError::invalid(format!(
                "image resource {} has an invalid PNG payload: {error}",
                self.id.as_str()
            ))
        })?;
        if reader.info().animation_control().is_some() {
            return Err(DrawingListError::invalid(format!(
                "image resource {} must not contain animated PNG data",
                self.id.as_str()
            ))
            .into());
        }
        let info = reader.info();
        if info.width != self.pixel_width || info.height != self.pixel_height {
            return Err(DrawingListError::invalid(format!(
                "image resource {} declares {}x{} pixels but contains {}x{} pixels",
                self.id.as_str(),
                self.pixel_width,
                self.pixel_height,
                info.width,
                info.height
            ))
            .into());
        }
        let actual_has_alpha =
            matches!(info.color_type, ColorType::GrayscaleAlpha | ColorType::Rgba)
                || info.trns.is_some();
        if actual_has_alpha != self.has_alpha {
            return Err(DrawingListError::invalid(format!(
                "image resource {} declares has_alpha={} but its PNG payload has_alpha={}",
                self.id.as_str(),
                self.has_alpha,
                actual_has_alpha
            ))
            .into());
        }
        loop {
            control.checkpoint()?;
            if reader
                .next_row()
                .map_err(|error| {
                    DrawingListError::invalid(format!(
                        "image resource {} has invalid PNG pixel data: {error}",
                        self.id.as_str()
                    ))
                })?
                .is_none()
            {
                break;
            }
        }
        reader.finish().map_err(|error| {
            DrawingListError::invalid(format!(
                "image resource {} has an incomplete or corrupt PNG payload: {error}",
                self.id.as_str()
            ))
        })?;
        drop(reader);
        if cursor.position() != self.image.data.len() as u64 {
            return Err(DrawingListError::invalid(format!(
                "image resource {} contains data after its PNG IEND chunk",
                self.id.as_str()
            ))
            .into());
        }
        Ok(ResourceValidationUsage {
            image_bytes: self.image.data.len(),
            image_pixels: header_pixels,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Paint {
    Solid {
        color: Color,
        /// Multiplies the intrinsic color alpha without quantizing either factor.
        #[serde(default = "opaque_paint", skip_serializing_if = "is_opaque_paint")]
        opacity: f64,
    },
    Resource {
        id: ResourceId,
        /// Multiplies the sampled resource alpha for this paint use.
        #[serde(default = "opaque_paint", skip_serializing_if = "is_opaque_paint")]
        opacity: f64,
    },
}

impl Paint {
    pub fn solid(color: Color) -> Self {
        Self::Solid {
            color,
            opacity: 1.0,
        }
    }

    pub fn resource(id: ResourceId) -> Self {
        Self::Resource { id, opacity: 1.0 }
    }

    pub fn opacity(&self) -> f64 {
        match self {
            Self::Solid { opacity, .. } | Self::Resource { opacity, .. } => *opacity,
        }
    }

    /// Sets paint opacity; document validation requires a finite value in `0..=1`.
    pub fn with_opacity(mut self, opacity: f64) -> Self {
        match &mut self {
            Self::Solid { opacity: value, .. } | Self::Resource { opacity: value, .. } => {
                *value = opacity;
            }
        }
        self
    }
}

fn opaque_paint() -> f64 {
    1.0
}

fn is_opaque_paint(opacity: &f64) -> bool {
    *opacity == 1.0
}

mod base64_bytes {
    use super::*;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&Base64Display::new(bytes, &STANDARD))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CanonicalBase64Visitor)
    }

    struct CanonicalBase64Visitor;

    impl<'de> Visitor<'de> for CanonicalBase64Visitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a canonical padded Base64 string")
        }

        fn visit_borrowed_str<E>(self, encoded: &'de str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            decode_canonical_base64(encoded).map_err(E::custom)
        }

        fn visit_str<E>(self, encoded: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            decode_canonical_base64(encoded).map_err(E::custom)
        }

        fn visit_string<E>(self, encoded: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            decode_canonical_base64(&encoded).map_err(E::custom)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalBase64Error {
    InvalidLength,
    NotCanonical,
    LengthOverflow,
}

impl fmt::Display for CanonicalBase64Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLength => "encoded asset data must use padded Base64",
            Self::NotCanonical => "encoded asset data is not canonical Base64",
            Self::LengthOverflow => "encoded asset byte count overflows usize",
        })
    }
}

/// Returns the decoded length of canonical standard Base64 without allocating decoded storage.
///
/// Besides alphabet and padding placement, this rejects non-zero unused bits in the final
/// sextet. Accepting those alternative spellings would make canonical JSON have more than one
/// wire representation for the same byte sequence.
pub(crate) fn canonical_base64_decoded_len(encoded: &str) -> Result<usize, CanonicalBase64Error> {
    let bytes = encoded.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(CanonicalBase64Error::InvalidLength);
    }
    let padding = if bytes.ends_with(b"==") {
        2
    } else if bytes.ends_with(b"=") {
        1
    } else {
        0
    };
    let content_len = bytes.len().saturating_sub(padding);
    if bytes[..content_len]
        .iter()
        .any(|byte| standard_base64_value(*byte).is_none())
        || bytes[content_len..].iter().any(|byte| *byte != b'=')
        || (padding == 1 && content_len % 4 != 3)
        || (padding == 2 && content_len % 4 != 2)
    {
        return Err(CanonicalBase64Error::NotCanonical);
    }

    let trailing_value = content_len
        .checked_sub(1)
        .and_then(|index| bytes.get(index))
        .and_then(|byte| standard_base64_value(*byte));
    if matches!((padding, trailing_value), (1, Some(value)) if value & 0b11 != 0)
        || matches!((padding, trailing_value), (2, Some(value)) if value & 0b1111 != 0)
    {
        return Err(CanonicalBase64Error::NotCanonical);
    }

    (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|decoded| decoded.checked_sub(padding))
        .ok_or(CanonicalBase64Error::LengthOverflow)
}

fn decode_canonical_base64(encoded: &str) -> Result<Vec<u8>, CanonicalBase64Error> {
    let expected_len = canonical_base64_decoded_len(encoded)?;
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| CanonicalBase64Error::NotCanonical)?;
    if decoded.len() != expected_len {
        return Err(CanonicalBase64Error::NotCanonical);
    }
    Ok(decoded)
}

fn standard_base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
