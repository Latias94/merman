use crate::{
    DRAWING_LIST_VERSION, DrawingCommand, DrawingListError, DrawingListPolicy, DrawingResource,
    PathSegment, Point, Rect, ResourceId, TextObligation,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSystem {
    LogicalPixelsYDown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub bounds: Rect,
}

impl Viewport {
    pub const fn new(bounds: Rect) -> Self {
        Self { bounds }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAnnotation {
    pub id: String,
    pub role: SemanticRole,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    Document,
    Node,
    Edge,
    Group,
    Label,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualSource {
    pub family: String,
    pub element_id: Option<String>,
    pub effect: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterFallback {
    pub id: String,
    pub image: ResourceId,
    pub bounds: Rect,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub reason: FallbackReason,
    pub source: VisualSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    ForeignObject,
    Filter,
    Mask,
    UnsupportedEffect,
    TextShaping,
    EmbeddedGraphic,
    HostResource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawingListDocument {
    pub version: u32,
    pub coordinate_system: CoordinateSystem,
    pub viewport: Viewport,
    pub policy: DrawingListPolicy,
    pub resources: Vec<DrawingResource>,
    pub commands: Vec<DrawingCommand>,
    pub semantics: Vec<SemanticAnnotation>,
    pub fallbacks: Vec<RasterFallback>,
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawingListLimits {
    pub max_serialized_bytes: usize,
    pub max_commands: usize,
    pub max_resources: usize,
    pub max_path_segments: usize,
    pub max_image_bytes: usize,
    pub max_image_pixels: usize,
    pub max_nesting_depth: usize,
    pub max_fallbacks: usize,
}

impl Default for DrawingListLimits {
    fn default() -> Self {
        Self {
            max_serialized_bytes: 128 * 1024 * 1024,
            max_commands: 1_000_000,
            max_resources: 100_000,
            max_path_segments: 2_000_000,
            max_image_bytes: 64 * 1024 * 1024,
            max_image_pixels: 64 * 1024 * 1024,
            max_nesting_depth: 256,
            max_fallbacks: 100_000,
        }
    }
}

impl DrawingListDocument {
    pub fn validate(&self) -> Result<(), DrawingListError> {
        self.validate_with_limits(&DrawingListLimits::default())
    }

    pub fn validate_with_limits(&self, limits: &DrawingListLimits) -> Result<(), DrawingListError> {
        if self.version != DRAWING_LIST_VERSION {
            return Err(DrawingListError::UnsupportedVersion {
                actual: self.version,
                expected: DRAWING_LIST_VERSION,
            });
        }
        if !matches!(self.coordinate_system, CoordinateSystem::LogicalPixelsYDown) {
            return Err(DrawingListError::invalid("unsupported coordinate system"));
        }
        if !self.viewport.bounds.is_valid() {
            return Err(DrawingListError::invalid("viewport bounds are invalid"));
        }
        validate_count("commands", self.commands.len(), limits.max_commands)?;
        validate_count("resources", self.resources.len(), limits.max_resources)?;
        validate_count("fallbacks", self.fallbacks.len(), limits.max_fallbacks)?;
        validate_extensions(&self.extensions)?;

        let mut resource_ids = BTreeSet::new();
        let mut paint_ids = BTreeSet::new();
        let mut image_pixels = 0usize;
        for resource in &self.resources {
            if !resource_ids.insert(resource.id().clone()) {
                return Err(DrawingListError::invalid(format!(
                    "duplicate resource id {}",
                    resource.id().as_str()
                )));
            }
            resource.validate(limits.max_path_segments, limits.max_image_bytes)?;
            if let DrawingResource::LinearGradient(gradient) = resource {
                paint_ids.insert(gradient.id.clone());
            }
            if let DrawingResource::Image(image) = resource {
                image_pixels = image_pixels
                    .checked_add(pixel_count(image.pixel_width, image.pixel_height)?)
                    .ok_or_else(|| {
                        DrawingListError::invalid("image pixel count overflows usize")
                    })?;
            }
        }
        validate_count("image_pixels", image_pixels, limits.max_image_pixels)?;

        let image_ids = self
            .resources
            .iter()
            .filter_map(|resource| match resource {
                DrawingResource::Image(image) => Some(image.id.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let path_ids = self
            .resources
            .iter()
            .filter_map(|resource| match resource {
                DrawingResource::Path(path) => Some(path.id.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        let mut semantic_ids = BTreeSet::new();
        for semantic in &self.semantics {
            if semantic.id.is_empty() || !semantic_ids.insert(semantic.id.as_str()) {
                return Err(DrawingListError::invalid(
                    "semantic ids must be non-empty and unique",
                ));
            }
        }

        let mut fallback_ids = BTreeSet::new();
        for fallback in &self.fallbacks {
            if fallback.id.is_empty() || !fallback_ids.insert(fallback.id.as_str()) {
                return Err(DrawingListError::invalid(
                    "fallback ids must be non-empty and unique",
                ));
            }
            if !image_ids.contains(&fallback.image) {
                return Err(DrawingListError::invalid(format!(
                    "fallback {} references a non-image resource",
                    fallback.id
                )));
            }
            if !fallback.bounds.is_valid()
                || fallback.pixel_width == 0
                || fallback.pixel_height == 0
                || fallback.source.family.is_empty()
                || fallback.source.effect.is_empty()
            {
                return Err(DrawingListError::invalid(format!(
                    "fallback {} is missing bounds, pixels, reason provenance, or source identity",
                    fallback.id
                )));
            }
            image_pixels = image_pixels
                .checked_add(pixel_count(fallback.pixel_width, fallback.pixel_height)?)
                .ok_or_else(|| DrawingListError::invalid("image pixel count overflows usize"))?;
            validate_count("image_pixels", image_pixels, limits.max_image_pixels)?;
        }

        let mut referenced_fallbacks = BTreeSet::new();
        let mut state_depth = 0usize;
        let mut semantic_depth = 0usize;
        for command in &self.commands {
            command.validate_numbers()?;
            match command {
                DrawingCommand::Save => {
                    state_depth = state_depth
                        .checked_add(1)
                        .ok_or_else(|| DrawingListError::invalid("state depth overflows usize"))?;
                    validate_count("nesting_depth", state_depth, limits.max_nesting_depth)?;
                }
                DrawingCommand::Restore => {
                    state_depth = state_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid("restore without matching save")
                    })?;
                }
                DrawingCommand::DrawPath { path, style } => {
                    if !path_ids.contains(path) {
                        return Err(DrawingListError::invalid(format!(
                            "draw_path references unknown path {}",
                            path.as_str()
                        )));
                    }
                    validate_paint(style.fill.as_ref(), &paint_ids)?;
                    if let Some(stroke) = &style.stroke {
                        validate_paint(Some(&stroke.paint), &paint_ids)?;
                    }
                }
                DrawingCommand::DrawImage { image, .. } => {
                    if !image_ids.contains(image) {
                        return Err(DrawingListError::invalid(format!(
                            "draw_image references unknown image {}",
                            image.as_str()
                        )));
                    }
                }
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    if !semantic_ids.contains(semantic_id.as_str()) {
                        return Err(DrawingListError::invalid(format!(
                            "semantic group references unknown id {semantic_id}"
                        )));
                    }
                    semantic_depth = semantic_depth.checked_add(1).ok_or_else(|| {
                        DrawingListError::invalid("semantic depth overflows usize")
                    })?;
                    validate_count("nesting_depth", semantic_depth, limits.max_nesting_depth)?;
                }
                DrawingCommand::EndSemanticGroup => {
                    semantic_depth = semantic_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid("semantic group end without matching begin")
                    })?;
                }
                DrawingCommand::DrawRasterSubtree { fallback_id } => {
                    if !fallback_ids.contains(fallback_id.as_str()) {
                        return Err(DrawingListError::invalid(format!(
                            "raster command references unknown fallback {fallback_id}"
                        )));
                    }
                    referenced_fallbacks.insert(fallback_id.as_str());
                }
                DrawingCommand::DrawText { run } => {
                    validate_paint(Some(&run.style.fill), &paint_ids)?;
                    match &run.obligation {
                        TextObligation::Outline { path } => {
                            if !path_ids.contains(path) {
                                return Err(DrawingListError::invalid(format!(
                                    "text outline references unknown path {}",
                                    path.as_str()
                                )));
                            }
                        }
                        TextObligation::RasterFallback { fallback_id } => {
                            if !fallback_ids.contains(fallback_id.as_str()) {
                                return Err(DrawingListError::invalid(format!(
                                    "text references unknown fallback {fallback_id}"
                                )));
                            }
                            referenced_fallbacks.insert(fallback_id.as_str());
                        }
                        TextObligation::HostText { .. } | TextObligation::GlyphRun { .. } => {}
                    }
                }
                DrawingCommand::SetOpacity { .. }
                | DrawingCommand::SetBlendMode { .. }
                | DrawingCommand::ConcatTransform { .. } => {}
            }
        }
        if state_depth != 0 {
            return Err(DrawingListError::invalid(
                "save/restore state is unbalanced",
            ));
        }
        if semantic_depth != 0 {
            return Err(DrawingListError::invalid("semantic groups are unbalanced"));
        }
        if referenced_fallbacks.len() != fallback_ids.len() {
            return Err(DrawingListError::invalid(
                "every raster fallback must be referenced by a drawing command",
            ));
        }
        if matches!(self.policy, DrawingListPolicy::VectorOnly) && !self.fallbacks.is_empty() {
            return Err(DrawingListError::invalid(
                "vector-only policy cannot contain raster fallbacks",
            ));
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, DrawingListError> {
        self.canonical_json_bytes_with_limits(&DrawingListLimits::default())
    }

    pub fn canonical_json_bytes_with_limits(
        &self,
        limits: &DrawingListLimits,
    ) -> Result<Vec<u8>, DrawingListError> {
        self.validate_with_limits(limits)?;
        let mut canonical = self.clone();
        canonical
            .resources
            .sort_by(|left, right| left.id().cmp(right.id()));
        canonical
            .semantics
            .sort_by(|left, right| left.id.cmp(&right.id));
        canonical
            .fallbacks
            .sort_by(|left, right| left.id.cmp(&right.id));
        let bytes = serde_json::to_vec(&canonical)
            .map_err(|error| DrawingListError::JsonEncode(error.to_string()))?;
        validate_count("serialized_bytes", bytes.len(), limits.max_serialized_bytes)?;
        Ok(bytes)
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DrawingListError> {
        Self::from_json_bytes_with_limits(bytes, &DrawingListLimits::default())
    }

    pub fn from_json_bytes_with_limits(
        bytes: &[u8],
        limits: &DrawingListLimits,
    ) -> Result<Self, DrawingListError> {
        validate_count("serialized_bytes", bytes.len(), limits.max_serialized_bytes)?;
        let document = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| DrawingListError::JsonDecode(error.to_string()))?;
        document.validate_with_limits(limits)?;
        Ok(document)
    }
}

fn validate_count(
    resource: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), DrawingListError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(DrawingListError::ResourceLimit {
            resource,
            actual,
            maximum,
        })
    }
}

fn validate_extensions(extensions: &BTreeMap<String, Value>) -> Result<(), DrawingListError> {
    if extensions.keys().any(|key| !key.starts_with("x-")) {
        return Err(DrawingListError::invalid(
            "only x-* metadata extensions are forward-compatible",
        ));
    }
    Ok(())
}

fn validate_paint(
    paint: Option<&crate::Paint>,
    paint_ids: &BTreeSet<ResourceId>,
) -> Result<(), DrawingListError> {
    if let Some(crate::Paint::Resource { id }) = paint {
        if !paint_ids.contains(id) {
            return Err(DrawingListError::invalid(format!(
                "paint references unknown paint resource {}",
                id.as_str()
            )));
        }
    }
    Ok(())
}

fn pixel_count(width: u32, height: u32) -> Result<usize, DrawingListError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| DrawingListError::invalid("image pixel count overflows usize"))
}

#[allow(dead_code)]
fn _path_segment_point(segment: &PathSegment) -> Option<Point> {
    match segment {
        PathSegment::MoveTo { to } | PathSegment::LineTo { to } => Some(*to),
        PathSegment::QuadTo { to, .. } | PathSegment::CubicTo { to, .. } => Some(*to),
        PathSegment::Close => None,
    }
}
