use super::{
    optional_string, required_bool, required_f64, required_string, required_u32, required_u64,
    required_value,
};
use crate::BindingError;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingOutputPlan {
    Ascii(Box<BindingAsciiOutputPlan>),
    Raster(BindingRasterOutputPlan),
    PdfFilterImages(BindingPdfFilterImageOutputPlan),
    Unknown(BindingUnknownOutputPlan),
}

impl BindingOutputPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::Ascii(_) => "ascii",
            Self::Raster(_) => "raster",
            Self::PdfFilterImages(_) => "pdf-filter-images",
            Self::Unknown(plan) => plan.kind(),
        }
    }
}

impl Serialize for BindingOutputPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Ascii(plan) => BindingAsciiOutputPlanWire {
                kind: "ascii",
                plan,
            }
            .serialize(serializer),
            Self::Raster(plan) => BindingRasterOutputPlanWire {
                kind: "raster",
                plan,
            }
            .serialize(serializer),
            Self::PdfFilterImages(plan) => BindingPdfFilterImageOutputPlanWire {
                kind: "pdf-filter-images",
                plan,
            }
            .serialize(serializer),
            Self::Unknown(plan) => plan.serialize(serializer),
        }
    }
}

/// Logical ASCII report carried by the canonical `ascii` operation.
///
/// The operation data contains the exact encoded text bytes. This plan carries only the
/// provider-neutral metadata needed by a host to make an emission decision without remeasuring
/// ANSI/HTML output or parsing the text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingAsciiOutputPlan {
    schema_version: u16,
    family: String,
    projection: String,
    encoding: String,
    primary_width: Option<u64>,
    primary_height: Option<u64>,
    emitted_width: u64,
    emitted_height: u64,
    width_profile: String,
    layout_profile: Option<String>,
    requested_layout_profile: String,
    requested_graph_presentation: String,
    compact_attempted: bool,
    requested_max_width: Option<u64>,
    primary_overflowed: Option<bool>,
    emitted_overflowed: bool,
    outcome: String,
    fallback_capability: String,
    fallback_attempted: bool,
    fallback_reason: Option<String>,
    trimmed: bool,
    lossiness: String,
}

#[derive(Serialize)]
struct BindingAsciiOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingAsciiOutputPlan,
}

impl BindingAsciiOutputPlan {
    #[cfg(feature = "ascii")]
    pub(crate) fn from_output(output: &merman::ascii::AsciiOutput) -> Self {
        Self::from_metadata(output.metadata())
    }

    #[cfg(feature = "ascii")]
    pub(super) fn from_metadata(metadata: merman::ascii::AsciiOutputMetadata) -> Self {
        Self {
            schema_version: metadata.schema_version,
            family: metadata.family,
            projection: metadata.projection,
            encoding: metadata.encoding,
            primary_width: metadata.primary_width,
            primary_height: metadata.primary_height,
            emitted_width: metadata.emitted_width,
            emitted_height: metadata.emitted_height,
            width_profile: metadata.width_profile,
            layout_profile: metadata.layout_profile,
            requested_layout_profile: metadata.requested_layout_profile,
            requested_graph_presentation: metadata.requested_graph_presentation,
            compact_attempted: metadata.compact_attempted,
            requested_max_width: metadata.requested_max_width,
            primary_overflowed: metadata.primary_overflowed,
            emitted_overflowed: metadata.emitted_overflowed,
            outcome: metadata.outcome,
            fallback_capability: metadata.fallback_capability,
            fallback_attempted: metadata.fallback_attempted,
            fallback_reason: metadata.fallback_reason,
            trimmed: metadata.trimmed,
            lossiness: metadata.lossiness,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    #[must_use]
    pub fn projection(&self) -> &str {
        &self.projection
    }

    #[must_use]
    pub fn encoding(&self) -> &str {
        &self.encoding
    }

    /// Measured primary width, unavailable when planning produced no primary geometry.
    #[must_use]
    pub const fn primary_width(&self) -> Option<u64> {
        self.primary_width
    }

    /// Measured primary height, with the same availability as [`Self::primary_width`].
    #[must_use]
    pub const fn primary_height(&self) -> Option<u64> {
        self.primary_height
    }

    #[must_use]
    pub const fn emitted_width(&self) -> u64 {
        self.emitted_width
    }

    #[must_use]
    pub const fn emitted_height(&self) -> u64 {
        self.emitted_height
    }

    #[must_use]
    pub fn width_profile(&self) -> &str {
        &self.width_profile
    }

    /// Effective primary layout, unavailable when no primary geometry exists.
    #[must_use]
    pub fn layout_profile(&self) -> Option<&str> {
        self.layout_profile.as_deref()
    }

    /// Layout profile requested by the host, including automatic selection.
    #[must_use]
    pub fn requested_layout_profile(&self) -> &str {
        &self.requested_layout_profile
    }

    /// Requested graph policy; [`Self::projection`] records the emitted presentation.
    #[must_use]
    pub fn requested_graph_presentation(&self) -> &str {
        &self.requested_graph_presentation
    }

    /// Whether automatic selection attempted a Compact candidate.
    #[must_use]
    pub const fn compact_attempted(&self) -> bool {
        self.compact_attempted
    }

    #[must_use]
    pub const fn requested_max_width(&self) -> Option<u64> {
        self.requested_max_width
    }

    /// Primary width overflow, unavailable when no primary geometry was measured.
    #[must_use]
    pub const fn primary_overflowed(&self) -> Option<bool> {
        self.primary_overflowed
    }

    /// Whether the emitted output exceeds the requested width.
    #[must_use]
    pub const fn emitted_overflowed(&self) -> bool {
        self.emitted_overflowed
    }

    #[must_use]
    pub fn outcome(&self) -> &str {
        &self.outcome
    }

    #[must_use]
    pub fn fallback_capability(&self) -> &str {
        &self.fallback_capability
    }

    #[must_use]
    pub const fn fallback_attempted(&self) -> bool {
        self.fallback_attempted
    }

    #[must_use]
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }

    #[must_use]
    pub const fn trimmed(&self) -> bool {
        self.trimmed
    }

    #[must_use]
    pub fn lossiness(&self) -> &str {
        &self.lossiness
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingRasterOutputPlan {
    pub(super) requested_width_px: f64,
    pub(super) requested_height_px: f64,
    pub(super) width_px: u32,
    pub(super) height_px: u32,
    pub(super) requested_scale: f64,
    pub(super) effective_scale: f64,
    pub(super) limited: bool,
}

#[derive(Serialize)]
struct BindingRasterOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingRasterOutputPlan,
}

impl BindingRasterOutputPlan {
    #[must_use]
    pub const fn requested_width_px(&self) -> f64 {
        self.requested_width_px
    }

    #[must_use]
    pub const fn requested_height_px(&self) -> f64 {
        self.requested_height_px
    }

    #[must_use]
    pub const fn width_px(&self) -> u32 {
        self.width_px
    }

    #[must_use]
    pub const fn height_px(&self) -> u32 {
        self.height_px
    }

    #[must_use]
    pub const fn requested_scale(&self) -> f64 {
        self.requested_scale
    }

    #[must_use]
    pub const fn effective_scale(&self) -> f64 {
        self.effective_scale
    }

    #[must_use]
    pub const fn limited(&self) -> bool {
        self.limited
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingPdfFilterImageOutputPlan {
    pub(super) filtered_groups: u64,
    pub(super) requested_scale: f32,
    pub(super) effective_scale: f32,
    pub(super) requested_image_pixels: u64,
    pub(super) effective_image_pixels: u64,
    pub(super) limited: bool,
}

#[derive(Serialize)]
struct BindingPdfFilterImageOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingPdfFilterImageOutputPlan,
}

impl BindingPdfFilterImageOutputPlan {
    #[must_use]
    pub const fn filtered_groups(&self) -> u64 {
        self.filtered_groups
    }

    #[must_use]
    pub const fn requested_scale(&self) -> f32 {
        self.requested_scale
    }

    #[must_use]
    pub const fn effective_scale(&self) -> f32 {
        self.effective_scale
    }

    #[must_use]
    pub const fn requested_image_pixels(&self) -> u64 {
        self.requested_image_pixels
    }

    #[must_use]
    pub const fn effective_image_pixels(&self) -> u64 {
        self.effective_image_pixels
    }

    #[must_use]
    pub const fn limited(&self) -> bool {
        self.limited
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingUnknownOutputPlan {
    kind: String,
    value: Value,
}

impl BindingUnknownOutputPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

impl Serialize for BindingUnknownOutputPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.value.serialize(serializer)
    }
}

pub(super) fn parse_output_plan(value: &Value) -> Result<BindingOutputPlan, BindingError> {
    let object = value.as_object().ok_or_else(|| {
        BindingError::invalid_argument(
            "invalid operation metadata JSON: output_plan must be an object",
        )
    })?;
    let kind = required_string(object, "kind")?;
    match kind {
        "ascii" => parse_ascii_output_plan(object)
            .map(Box::new)
            .map(BindingOutputPlan::Ascii),
        "raster" => Ok(BindingOutputPlan::Raster(BindingRasterOutputPlan {
            requested_width_px: required_f64(object, "requested_width_px")?,
            requested_height_px: required_f64(object, "requested_height_px")?,
            width_px: required_u32(object, "width_px")?,
            height_px: required_u32(object, "height_px")?,
            requested_scale: required_f64(object, "requested_scale")?,
            effective_scale: required_f64(object, "effective_scale")?,
            limited: required_bool(object, "limited")?,
        })),
        "pdf-filter-images" => Ok(BindingOutputPlan::PdfFilterImages(
            BindingPdfFilterImageOutputPlan {
                filtered_groups: required_u64(object, "filtered_groups")?,
                requested_scale: required_f32(object, "requested_scale")?,
                effective_scale: required_f32(object, "effective_scale")?,
                requested_image_pixels: required_u64(object, "requested_image_pixels")?,
                effective_image_pixels: required_u64(object, "effective_image_pixels")?,
                limited: required_bool(object, "limited")?,
            },
        )),
        _ => Ok(BindingOutputPlan::Unknown(BindingUnknownOutputPlan {
            kind: kind.to_owned(),
            value: value.clone(),
        })),
    }
}

fn parse_ascii_output_plan(
    object: &Map<String, Value>,
) -> Result<BindingAsciiOutputPlan, BindingError> {
    let schema_version = required_u16(object, "schema_version")?;
    if schema_version != 4 {
        return Err(BindingError::invalid_argument(format!(
            "unsupported ASCII output schema version {schema_version}; expected 4"
        )));
    }
    let plan = BindingAsciiOutputPlan {
        schema_version,
        family: required_string(object, "family")?.to_owned(),
        projection: required_string(object, "projection")?.to_owned(),
        encoding: required_string(object, "encoding")?.to_owned(),
        primary_width: required_nullable_u64(object, "primary_width")?,
        primary_height: required_nullable_u64(object, "primary_height")?,
        emitted_width: required_u64(object, "emitted_width")?,
        emitted_height: required_u64(object, "emitted_height")?,
        width_profile: required_string(object, "width_profile")?.to_owned(),
        layout_profile: required_nullable_string(object, "layout_profile")?.map(str::to_owned),
        requested_layout_profile: required_string(object, "requested_layout_profile")?.to_owned(),
        requested_graph_presentation: required_string(object, "requested_graph_presentation")?
            .to_owned(),
        compact_attempted: required_bool(object, "compact_attempted")?,
        requested_max_width: optional_u64(object, "requested_max_width")?,
        primary_overflowed: required_nullable_bool(object, "primary_overflowed")?,
        emitted_overflowed: required_bool(object, "emitted_overflowed")?,
        outcome: required_string(object, "outcome")?.to_owned(),
        fallback_capability: required_string(object, "fallback_capability")?.to_owned(),
        fallback_attempted: required_bool(object, "fallback_attempted")?,
        fallback_reason: optional_string(object, "fallback_reason")?.map(str::to_owned),
        trimmed: required_bool(object, "trimmed")?,
        lossiness: required_string(object, "lossiness")?.to_owned(),
    };
    let primary_available = plan.primary_width.is_some();
    if plan.primary_height.is_some() != primary_available
        || plan.layout_profile.is_some() != primary_available
        || plan.primary_overflowed.is_some() != primary_available
    {
        return Err(BindingError::invalid_argument(
            "invalid operation metadata JSON: primary_width, primary_height, layout_profile and primary_overflowed must all be null or all be non-null",
        ));
    }
    Ok(plan)
}

fn required_u16(object: &Map<String, Value>, field: &str) -> Result<u16, BindingError> {
    let value = required_u64(object, field)?;
    u16::try_from(value).map_err(|_| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` exceeds unsigned 16-bit range"
        ))
    })
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, BindingError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            BindingError::invalid_argument(format!(
                "invalid operation metadata JSON: `{field}` must be an unsigned 64-bit integer or null"
            ))
        }),
    }
}

fn required_nullable_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, BindingError> {
    required_value(object, field)?;
    optional_u64(object, field)
}

fn required_nullable_bool(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<bool>, BindingError> {
    if required_value(object, field)?.is_null() {
        Ok(None)
    } else {
        required_bool(object, field).map(Some)
    }
}

fn required_nullable_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, BindingError> {
    required_value(object, field)?;
    optional_string(object, field)
}

fn required_f32(object: &Map<String, Value>, field: &str) -> Result<f32, BindingError> {
    let value = required_f64(object, field)?;
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` exceeds finite 32-bit float range"
        )));
    }
    Ok(value as f32)
}
