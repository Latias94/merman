use crate::{
    DRAWING_LIST_VERSION, DrawingCommand, DrawingListError, DrawingListPolicy, DrawingResource,
    PathSegment, Point, Rect, ResourceId, TextObligation,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

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
    pub scale: f64,
    pub format: RasterFormat,
    pub alpha: AlphaMode,
    pub reason: FallbackReason,
    pub source: VisualSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterFormat {
    Png,
}

impl RasterFormat {
    pub(crate) fn matches_media_type(self, media_type: &str) -> bool {
        match self {
            Self::Png => crate::resources::media_type_matches(media_type, "image", "png"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    Opaque,
    Straight,
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
    Pattern,
    Math,
    Icon,
    HandDrawn,
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
    pub max_fallback_pixels: usize,
    pub max_font_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_fallbacks: usize,
    pub max_text_bytes: usize,
    pub max_glyphs: usize,
}

/// Cumulative footprint of one validated drawing document.
///
/// The counts are intentionally kept separate from [`DrawingListLimits`].  Limits are a caller
/// policy (and may be stricter than the protocol defaults), while a footprint is an observation
/// that the renderer can charge to its existing operation work budget before serializing bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawingListFootprint {
    pub commands: usize,
    pub resources: usize,
    pub semantics: usize,
    pub fallbacks: usize,
    pub path_segments: usize,
    pub gradient_stops: usize,
    pub image_bytes: usize,
    pub image_pixels: usize,
    pub fallback_pixels: usize,
    pub font_bytes: usize,
    pub text_bytes: usize,
    pub glyphs: usize,
    pub max_nesting_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupScope {
    Layer,
    Semantic,
    Clip,
}

impl DrawingListFootprint {
    /// Checks every cumulative budget represented by this footprint.
    ///
    /// The serialized-byte ceiling is intentionally omitted: only the bounded JSON writer can
    /// know the exact encoded size.  Callers that need to charge work before serialization can
    /// use this method first, then rely on [`DrawingListDocument::canonical_json_bytes_with_limits`]
    /// for the final byte-accurate check.
    pub fn check_limits(&self, limits: &DrawingListLimits) -> Result<(), DrawingListError> {
        validate_count("commands", self.commands, limits.max_commands)?;
        validate_count("resources", self.resources, limits.max_resources)?;
        validate_count("fallbacks", self.fallbacks, limits.max_fallbacks)?;
        validate_count(
            "path_segments",
            self.path_segments,
            limits.max_path_segments,
        )?;
        validate_count("image_bytes", self.image_bytes, limits.max_image_bytes)?;
        validate_count("image_pixels", self.image_pixels, limits.max_image_pixels)?;
        validate_count(
            "fallback_pixels",
            self.fallback_pixels,
            limits.max_fallback_pixels,
        )?;
        validate_count("font_bytes", self.font_bytes, limits.max_font_bytes)?;
        validate_count(
            "nesting_depth",
            self.max_nesting_depth,
            limits.max_nesting_depth,
        )?;
        validate_count("text_bytes", self.text_bytes, limits.max_text_bytes)?;
        validate_count("glyphs", self.glyphs, limits.max_glyphs)?;
        Ok(())
    }

    /// Converts the footprint to conservative operation work units.
    ///
    /// Structural items, path segments, and glyphs cost one unit each.  Inline bytes and pixels
    /// cost one unit per started KiB; their exact safety ceilings remain enforced by the protocol
    /// validator.  Checked arithmetic keeps hostile or corrupted documents fail-closed.
    pub fn work_units(self) -> Result<usize, DrawingListError> {
        let mut units = 0usize;
        for value in [
            self.commands,
            self.resources,
            self.semantics,
            self.fallbacks,
            self.path_segments,
            self.gradient_stops,
            self.glyphs,
            self.max_nesting_depth,
        ] {
            units = units.checked_add(value).ok_or_else(|| {
                DrawingListError::invalid("DrawingList footprint overflows usize")
            })?;
        }
        for value in [
            self.image_bytes,
            self.image_pixels,
            self.fallback_pixels,
            self.font_bytes,
            self.text_bytes,
        ] {
            let kib = value / 1024 + usize::from(value % 1024 != 0);
            units = units.checked_add(kib).ok_or_else(|| {
                DrawingListError::invalid("DrawingList footprint overflows usize")
            })?;
        }
        Ok(units)
    }
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
            max_fallback_pixels: 64 * 1024 * 1024,
            max_font_bytes: 16 * 1024 * 1024,
            max_nesting_depth: 256,
            max_fallbacks: 100_000,
            max_text_bytes: 16 * 1024 * 1024,
            max_glyphs: 10_000_000,
        }
    }
}

impl DrawingListDocument {
    /// Computes the document-wide footprint used by renderer operation accounting.
    ///
    /// Callers normally invoke this after [`Self::validate_with_limits`].  The method still
    /// checks stack underflow and checked arithmetic so it never turns malformed input into an
    /// unbounded accounting value.
    pub fn footprint(&self) -> Result<DrawingListFootprint, DrawingListError> {
        let mut footprint = DrawingListFootprint {
            commands: self.commands.len(),
            resources: self.resources.len(),
            semantics: self.semantics.len(),
            fallbacks: self.fallbacks.len(),
            ..DrawingListFootprint::default()
        };

        for resource in &self.resources {
            match resource {
                DrawingResource::Path(path) => {
                    footprint.path_segments = footprint
                        .path_segments
                        .checked_add(path.segments.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList path segment footprint overflows usize",
                            )
                        })?;
                }
                DrawingResource::LinearGradient(gradient) => {
                    footprint.gradient_stops = footprint
                        .gradient_stops
                        .checked_add(gradient.stops.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList gradient footprint overflows usize",
                            )
                        })?;
                }
                DrawingResource::RadialGradient(gradient) => {
                    footprint.gradient_stops = footprint
                        .gradient_stops
                        .checked_add(gradient.stops.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList gradient footprint overflows usize",
                            )
                        })?;
                }
                DrawingResource::Image(image) => {
                    footprint.image_bytes = footprint
                        .image_bytes
                        .checked_add(image.image.data.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList image-byte footprint overflows usize",
                            )
                        })?;
                    footprint.image_pixels = footprint
                        .image_pixels
                        .checked_add(pixel_count(image.pixel_width, image.pixel_height)?)
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList image-pixel footprint overflows usize",
                            )
                        })?;
                }
                DrawingResource::Pattern(_) => {}
                DrawingResource::Font(font) => {
                    footprint.font_bytes = footprint
                        .font_bytes
                        .checked_add(font.font.data.len())
                        .ok_or_else(|| {
                        DrawingListError::invalid("DrawingList font-byte footprint overflows usize")
                    })?;
                }
            }
        }

        for fallback in &self.fallbacks {
            footprint.fallback_pixels = footprint
                .fallback_pixels
                .checked_add(pixel_count(fallback.pixel_width, fallback.pixel_height)?)
                .ok_or_else(|| {
                    DrawingListError::invalid(
                        "DrawingList fallback-pixel footprint overflows usize",
                    )
                })?;
        }

        let mut state_depth = 0usize;
        let mut semantic_depth = 0usize;
        let mut layer_depth = 0usize;
        let mut clip_depth = 0usize;
        let mut state_scopes = Vec::new();
        let mut group_scopes = Vec::new();
        for command in &self.commands {
            match command {
                DrawingCommand::Save => {
                    state_depth = state_depth.checked_add(1).ok_or_else(|| {
                        DrawingListError::invalid("DrawingList state footprint overflows usize")
                    })?;
                    state_scopes.push((
                        semantic_depth,
                        layer_depth,
                        clip_depth,
                        group_scopes.len(),
                    ));
                }
                DrawingCommand::Restore => {
                    let Some((saved_semantic, saved_layer, saved_clip, saved_group_len)) =
                        state_scopes.pop()
                    else {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found restore without save",
                        ));
                    };
                    if semantic_depth != saved_semantic || layer_depth != saved_layer {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found restore across an open group",
                        ));
                    }
                    if clip_depth < saved_clip {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found an invalid clip scope",
                        ));
                    }
                    while group_scopes.len() > saved_group_len {
                        match group_scopes.pop() {
                            Some(GroupScope::Clip) => {}
                            Some(GroupScope::Layer | GroupScope::Semantic) => {
                                return Err(DrawingListError::invalid(
                                    "DrawingList footprint found restore across an open group",
                                ));
                            }
                            None => break,
                        }
                    }
                    clip_depth = saved_clip;
                    state_depth = state_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid("DrawingList state footprint underflows usize")
                    })?;
                }
                DrawingCommand::BeginLayer { .. } => {
                    layer_depth = layer_depth.checked_add(1).ok_or_else(|| {
                        DrawingListError::invalid("DrawingList layer footprint overflows usize")
                    })?;
                    group_scopes.push(GroupScope::Layer);
                }
                DrawingCommand::EndLayer => {
                    if !matches!(group_scopes.pop(), Some(GroupScope::Layer)) {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found layer end out of order",
                        ));
                    }
                    layer_depth = layer_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid(
                            "DrawingList footprint found layer end without begin",
                        )
                    })?;
                }
                DrawingCommand::ClipPath { .. } => {
                    if state_depth == 0 {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found clip path without save",
                        ));
                    }
                    clip_depth = clip_depth.checked_add(1).ok_or_else(|| {
                        DrawingListError::invalid("DrawingList clip footprint overflows usize")
                    })?;
                    group_scopes.push(GroupScope::Clip);
                }
                DrawingCommand::BeginSemanticGroup { .. } => {
                    semantic_depth = semantic_depth.checked_add(1).ok_or_else(|| {
                        DrawingListError::invalid("DrawingList semantic footprint overflows usize")
                    })?;
                    group_scopes.push(GroupScope::Semantic);
                }
                DrawingCommand::EndSemanticGroup => {
                    if !matches!(group_scopes.pop(), Some(GroupScope::Semantic)) {
                        return Err(DrawingListError::invalid(
                            "DrawingList footprint found semantic group end out of order",
                        ));
                    }
                    semantic_depth = semantic_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid(
                            "DrawingList footprint found semantic end without begin",
                        )
                    })?;
                }
                DrawingCommand::DrawText { run } => {
                    footprint.text_bytes = footprint
                        .text_bytes
                        .checked_add(run.text.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid(
                                "DrawingList text-byte footprint overflows usize",
                            )
                        })?;
                    if let TextObligation::GlyphRun { glyphs } = &run.obligation {
                        footprint.glyphs =
                            footprint.glyphs.checked_add(glyphs.len()).ok_or_else(|| {
                                DrawingListError::invalid(
                                    "DrawingList glyph footprint overflows usize",
                                )
                            })?;
                    }
                }
                DrawingCommand::SetOpacity { .. }
                | DrawingCommand::SetBlendMode { .. }
                | DrawingCommand::ConcatTransform { .. }
                | DrawingCommand::DrawPath { .. }
                | DrawingCommand::DrawImage { .. }
                | DrawingCommand::DrawRasterSubtree { .. } => {}
            }
            let nesting = state_depth
                .checked_add(semantic_depth)
                .and_then(|value| value.checked_add(layer_depth))
                .and_then(|value| value.checked_add(clip_depth))
                .ok_or_else(|| {
                    DrawingListError::invalid("DrawingList nesting footprint overflows usize")
                })?;
            footprint.max_nesting_depth = footprint.max_nesting_depth.max(nesting);
        }
        if !state_scopes.is_empty()
            || state_depth != 0
            || semantic_depth != 0
            || layer_depth != 0
            || clip_depth != 0
            || !group_scopes.is_empty()
        {
            return Err(DrawingListError::invalid(
                "DrawingList footprint found unbalanced command scopes",
            ));
        }
        Ok(footprint)
    }

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
        let mut image_resources = BTreeMap::new();
        let mut paint_ids = BTreeSet::new();
        let mut font_ids = BTreeSet::new();
        let mut image_bytes = 0usize;
        let mut image_pixels = 0usize;
        let mut font_bytes = 0usize;
        let mut path_segments = 0usize;
        for resource in &self.resources {
            if !resource_ids.insert(resource.id().clone()) {
                return Err(DrawingListError::invalid(format!(
                    "duplicate resource id {}",
                    resource.id().as_str()
                )));
            }
            let usage = resource.validate(
                limits.max_path_segments,
                image_bytes,
                limits.max_image_bytes,
                image_pixels,
                limits.max_image_pixels,
                limits.max_font_bytes,
            )?;
            image_bytes = image_bytes
                .checked_add(usage.image_bytes)
                .ok_or_else(|| DrawingListError::invalid("image byte count overflows usize"))?;
            image_pixels = image_pixels
                .checked_add(usage.image_pixels)
                .ok_or_else(|| DrawingListError::invalid("image pixel count overflows usize"))?;
            if let DrawingResource::Path(path) = resource {
                path_segments =
                    path_segments
                        .checked_add(path.segments.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid("path segment count overflows usize")
                        })?;
                validate_count("path_segments", path_segments, limits.max_path_segments)?;
            }
            match resource {
                DrawingResource::LinearGradient(gradient) => {
                    paint_ids.insert(gradient.id.clone());
                }
                DrawingResource::RadialGradient(gradient) => {
                    paint_ids.insert(gradient.id.clone());
                }
                DrawingResource::Pattern(pattern) => {
                    paint_ids.insert(pattern.id.clone());
                }
                DrawingResource::Font(font) => {
                    font_ids.insert(font.id.clone());
                    font_bytes = font_bytes
                        .checked_add(font.font.data.len())
                        .ok_or_else(|| {
                            DrawingListError::invalid("font byte count overflows usize")
                        })?;
                    validate_count("font_bytes", font_bytes, limits.max_font_bytes)?;
                }
                DrawingResource::Path(_) | DrawingResource::Image(_) => {}
            }
            if let DrawingResource::Image(image) = resource {
                image_resources.insert(image.id.clone(), image);
            }
        }
        let image_ids = image_resources.keys().cloned().collect::<BTreeSet<_>>();

        for resource in &self.resources {
            if let DrawingResource::Pattern(pattern) = resource
                && !image_resources.contains_key(&pattern.image)
            {
                return Err(DrawingListError::invalid(format!(
                    "pattern {} references unknown image {}",
                    pattern.id.as_str(),
                    pattern.image.as_str()
                )));
            }
        }
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
        let mut fallback_pixels = 0usize;
        for fallback in &self.fallbacks {
            if fallback.id.is_empty() || !fallback_ids.insert(fallback.id.as_str()) {
                return Err(DrawingListError::invalid(
                    "fallback ids must be non-empty and unique",
                ));
            }
            let Some(image) = image_resources.get(&fallback.image).copied() else {
                return Err(DrawingListError::invalid(format!(
                    "fallback {} references a non-image resource",
                    fallback.id
                )));
            };
            if !fallback.bounds.is_valid()
                || fallback.pixel_width == 0
                || fallback.pixel_height == 0
                || !fallback.scale.is_finite()
                || fallback.scale <= 0.0
                || fallback.source.family.is_empty()
                || fallback.source.effect.is_empty()
                || image.pixel_width != fallback.pixel_width
                || image.pixel_height != fallback.pixel_height
                || !fallback.format.matches_media_type(&image.image.media_type)
                || !matches!(
                    (image.has_alpha, fallback.alpha),
                    (false, AlphaMode::Opaque) | (true, AlphaMode::Straight)
                )
            {
                return Err(DrawingListError::invalid(format!(
                    "fallback {} is missing valid bounds, pixels, scale, format, alpha mode, reason provenance, or source identity",
                    fallback.id
                )));
            }
            fallback_pixels = fallback_pixels
                .checked_add(pixel_count(fallback.pixel_width, fallback.pixel_height)?)
                .ok_or_else(|| DrawingListError::invalid("fallback pixel count overflows usize"))?;
            validate_count(
                "fallback_pixels",
                fallback_pixels,
                limits.max_fallback_pixels,
            )?;
        }

        let mut referenced_fallbacks = BTreeSet::new();
        let mut state_depth = 0usize;
        let mut state_scopes = Vec::new();
        let mut semantic_depth = 0usize;
        let mut layer_depth = 0usize;
        let mut clip_depth = 0usize;
        let mut group_scopes = Vec::new();
        let mut text_bytes = 0usize;
        let mut glyph_count = 0usize;
        for command in &self.commands {
            command.validate_numbers()?;
            match command {
                DrawingCommand::Save => {
                    state_depth = state_depth
                        .checked_add(1)
                        .ok_or_else(|| DrawingListError::invalid("state depth overflows usize"))?;
                    validate_count("nesting_depth", state_depth, limits.max_nesting_depth)?;
                    state_scopes.push((
                        semantic_depth,
                        layer_depth,
                        clip_depth,
                        group_scopes.len(),
                    ));
                }
                DrawingCommand::Restore => {
                    let Some((
                        saved_semantic_depth,
                        saved_layer_depth,
                        saved_clip_depth,
                        saved_group_len,
                    )) = state_scopes.pop()
                    else {
                        return Err(DrawingListError::invalid("restore without matching save"));
                    };
                    if semantic_depth != saved_semantic_depth {
                        return Err(DrawingListError::invalid(
                            "restore cannot cross an open semantic group",
                        ));
                    }
                    if layer_depth != saved_layer_depth {
                        return Err(DrawingListError::invalid(
                            "restore cannot cross an open layer",
                        ));
                    }
                    if clip_depth < saved_clip_depth {
                        return Err(DrawingListError::invalid(
                            "clip scope cannot end before its save scope",
                        ));
                    }
                    while group_scopes.len() > saved_group_len {
                        match group_scopes.pop() {
                            Some(GroupScope::Clip) => {}
                            Some(GroupScope::Layer | GroupScope::Semantic) => {
                                return Err(DrawingListError::invalid(
                                    "restore cannot cross an open semantic or layer group",
                                ));
                            }
                            None => break,
                        }
                    }
                    clip_depth = saved_clip_depth;
                    state_depth = state_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid("restore without matching save")
                    })?;
                }
                DrawingCommand::BeginLayer { .. } => {
                    layer_depth = layer_depth
                        .checked_add(1)
                        .ok_or_else(|| DrawingListError::invalid("layer depth overflows usize"))?;
                    validate_count("nesting_depth", layer_depth, limits.max_nesting_depth)?;
                    group_scopes.push(GroupScope::Layer);
                }
                DrawingCommand::EndLayer => {
                    if !matches!(group_scopes.pop(), Some(GroupScope::Layer)) {
                        return Err(DrawingListError::invalid(
                            "layer end does not match the open group",
                        ));
                    }
                    layer_depth = layer_depth.checked_sub(1).ok_or_else(|| {
                        DrawingListError::invalid("layer end without matching begin")
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
                DrawingCommand::ClipPath { path, .. } => {
                    if state_depth == 0 {
                        return Err(DrawingListError::invalid(
                            "clip_path must be scoped by save/restore",
                        ));
                    }
                    if !path_ids.contains(path) {
                        return Err(DrawingListError::invalid(format!(
                            "clip_path references unknown path {}",
                            path.as_str()
                        )));
                    }
                    clip_depth = clip_depth
                        .checked_add(1)
                        .ok_or_else(|| DrawingListError::invalid("clip depth overflows usize"))?;
                    validate_count("nesting_depth", clip_depth, limits.max_nesting_depth)?;
                    group_scopes.push(GroupScope::Clip);
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
                    group_scopes.push(GroupScope::Semantic);
                }
                DrawingCommand::EndSemanticGroup => {
                    if !matches!(group_scopes.pop(), Some(GroupScope::Semantic)) {
                        return Err(DrawingListError::invalid(
                            "semantic group end does not match the open group",
                        ));
                    }
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
                    text_bytes = text_bytes.checked_add(run.text.len()).ok_or_else(|| {
                        DrawingListError::invalid("text byte count overflows usize")
                    })?;
                    validate_count("text_bytes", text_bytes, limits.max_text_bytes)?;
                    validate_paint(Some(&run.style.fill), &paint_ids)?;
                    if let Some(font) = &run.style.font.resource
                        && !font_ids.contains(font)
                    {
                        return Err(DrawingListError::invalid(format!(
                            "text references unknown font resource {}",
                            font.as_str()
                        )));
                    }
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
                        TextObligation::GlyphRun { glyphs } => {
                            glyph_count =
                                glyph_count.checked_add(glyphs.len()).ok_or_else(|| {
                                    DrawingListError::invalid("glyph count overflows usize")
                                })?;
                            validate_count("glyphs", glyph_count, limits.max_glyphs)?;
                        }
                        TextObligation::HostText { .. } => {}
                    }
                }
                DrawingCommand::SetOpacity { .. }
                | DrawingCommand::SetBlendMode { .. }
                | DrawingCommand::ConcatTransform { .. } => {}
            }
            let nesting_depth = state_depth
                .checked_add(semantic_depth)
                .and_then(|depth| depth.checked_add(layer_depth))
                .and_then(|depth| depth.checked_add(clip_depth))
                .ok_or_else(|| DrawingListError::invalid("nesting depth overflows usize"))?;
            validate_count("nesting_depth", nesting_depth, limits.max_nesting_depth)?;
        }
        if state_depth != 0 {
            return Err(DrawingListError::invalid(
                "save/restore state is unbalanced",
            ));
        }
        if !state_scopes.is_empty() {
            return Err(DrawingListError::invalid(
                "save/restore scopes are unbalanced",
            ));
        }
        if !group_scopes.is_empty() {
            return Err(DrawingListError::invalid("group scopes are unbalanced"));
        }
        if clip_depth != 0 {
            return Err(DrawingListError::invalid("clip scopes are unbalanced"));
        }
        if semantic_depth != 0 {
            return Err(DrawingListError::invalid("semantic groups are unbalanced"));
        }
        if layer_depth != 0 {
            return Err(DrawingListError::invalid("layers are unbalanced"));
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
        // Canonical ordering must not require cloning the document: resources may contain large
        // embedded images or fonts, and the serialized-byte limit is specifically intended to
        // bound retained output.  Sort borrowed references so only the small index vectors are
        // allocated before the bounded writer starts emitting bytes.
        let mut resources = self.resources.iter().collect::<Vec<_>>();
        resources.sort_unstable_by(|left, right| left.id().cmp(right.id()));
        let mut semantics = self.semantics.iter().collect::<Vec<_>>();
        semantics.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let mut fallbacks = self.fallbacks.iter().collect::<Vec<_>>();
        fallbacks.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        let canonical = CanonicalDocument {
            document: self,
            resources,
            semantics,
            fallbacks,
        };
        let mut writer = LimitedWriter::new(limits.max_serialized_bytes);
        match serde_json::to_writer(&mut writer, &canonical) {
            Ok(()) => Ok(writer.into_inner()),
            Err(_error) if writer.exceeded() => Err(DrawingListError::ResourceLimit {
                resource: "serialized_bytes",
                actual: writer.attempted_bytes(),
                maximum: limits.max_serialized_bytes,
            }),
            Err(error) => Err(DrawingListError::JsonEncode(error.to_string())),
        }
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, DrawingListError> {
        Self::from_json_bytes_with_limits(bytes, &DrawingListLimits::default())
    }

    pub fn from_json_bytes_with_limits(
        bytes: &[u8],
        limits: &DrawingListLimits,
    ) -> Result<Self, DrawingListError> {
        validate_count("serialized_bytes", bytes.len(), limits.max_serialized_bytes)?;
        validate_encoded_asset_budgets(bytes, limits)?;
        let document = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| DrawingListError::JsonDecode(error.to_string()))?;
        document.validate_with_limits(limits)?;
        Ok(document)
    }
}

/// Borrowed canonical view used to serialize stable collection order without cloning embedded
/// resource payloads.  Its field order intentionally mirrors [`DrawingListDocument`]'s wire
/// representation.
struct CanonicalDocument<'a> {
    document: &'a DrawingListDocument,
    resources: Vec<&'a DrawingResource>,
    semantics: Vec<&'a SemanticAnnotation>,
    fallbacks: Vec<&'a RasterFallback>,
}

impl Serialize for CanonicalDocument<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("DrawingListDocument", 9)?;
        state.serialize_field("version", &self.document.version)?;
        state.serialize_field("coordinate_system", &self.document.coordinate_system)?;
        state.serialize_field("viewport", &self.document.viewport)?;
        state.serialize_field("policy", &self.document.policy)?;
        state.serialize_field("resources", &self.resources)?;
        state.serialize_field("commands", &self.document.commands)?;
        state.serialize_field("semantics", &self.semantics)?;
        state.serialize_field("fallbacks", &self.fallbacks)?;
        state.serialize_field(
            "extensions",
            &CanonicalJsonValue::Object(&self.document.extensions),
        )?;
        state.end()
    }
}

/// Borrowed recursive JSON view used for canonical extension serialization.
///
/// `DrawingListDocument::extensions` is keyed by a `BTreeMap`, but nested JSON values can use an
/// insertion-ordered object map when the `serde_json` preserve-order feature is enabled.  Sorting
/// every object level here keeps canonical bytes independent of parser or producer insertion
/// order without cloning extension payloads.
enum CanonicalJsonValue<'a> {
    Value(&'a Value),
    Object(&'a BTreeMap<String, Value>),
}

impl Serialize for CanonicalJsonValue<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{SerializeMap, SerializeSeq};

        match self {
            Self::Object(object) => {
                let mut map = serializer.serialize_map(Some(object.len()))?;
                for (key, value) in object.iter() {
                    map.serialize_entry(key, &CanonicalJsonValue::Value(value))?;
                }
                map.end()
            }
            Self::Value(Value::Null) => serializer.serialize_none(),
            Self::Value(Value::Bool(value)) => serializer.serialize_bool(*value),
            Self::Value(Value::Number(value)) => value.serialize(serializer),
            Self::Value(Value::String(value)) => serializer.serialize_str(value),
            Self::Value(Value::Array(values)) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&CanonicalJsonValue::Value(value))?;
                }
                sequence.end()
            }
            Self::Value(Value::Object(object)) => {
                let mut entries = object.iter().collect::<Vec<_>>();
                entries.sort_unstable_by_key(|(left, _)| *left);
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, &CanonicalJsonValue::Value(value))?;
                }
                map.end()
            }
        }
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

#[derive(Deserialize)]
struct EncodedAssetBudgetDocument<'a> {
    #[serde(borrow)]
    resources: Vec<&'a RawValue>,
}

#[derive(Deserialize)]
struct EncodedAssetBudgetResource<'a> {
    #[serde(borrow)]
    kind: Cow<'a, str>,
    #[serde(default, borrow)]
    image: Option<&'a RawValue>,
    #[serde(default, borrow)]
    font: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct EncodedAssetBudget<'a> {
    #[serde(borrow)]
    data: &'a RawValue,
}

/// Charges encoded assets before their Base64 payloads are materialized by the public model.
///
/// This is deliberately a narrow borrowed wire view rather than a second DrawingList decoder. It
/// leaves structural validation to Serde and [`DrawingListDocument::validate_with_limits`], while
/// making caller-selected image and font byte ceilings effective before decoded buffers allocate.
fn validate_encoded_asset_budgets(
    bytes: &[u8],
    limits: &DrawingListLimits,
) -> Result<(), DrawingListError> {
    let document = serde_json::from_slice::<EncodedAssetBudgetDocument<'_>>(bytes)
        .map_err(|error| DrawingListError::JsonDecode(error.to_string()))?;
    validate_count("resources", document.resources.len(), limits.max_resources)?;

    let mut image_bytes = 0usize;
    let mut font_bytes = 0usize;
    for raw in document.resources {
        let resource = serde_json::from_str::<EncodedAssetBudgetResource<'_>>(raw.get())
            .map_err(|error| DrawingListError::JsonDecode(error.to_string()))?;
        let (asset, total, maximum, label) = match resource.kind.as_ref() {
            "image" => (
                resource.image,
                &mut image_bytes,
                limits.max_image_bytes,
                "image_bytes",
            ),
            "font" => (
                resource.font,
                &mut font_bytes,
                limits.max_font_bytes,
                "font_bytes",
            ),
            _ => continue,
        };
        let Some(asset) = asset else {
            continue;
        };
        let asset = serde_json::from_str::<EncodedAssetBudget<'_>>(asset.get())
            .map_err(|error| DrawingListError::JsonDecode(error.to_string()))?;
        let decoded = canonical_base64_decoded_len(asset.data.get())?;
        *total = total
            .checked_add(decoded)
            .ok_or_else(|| DrawingListError::invalid(format!("{label} count overflows usize")))?;
        validate_count(label, *total, maximum)?;
    }
    Ok(())
}

fn canonical_base64_decoded_len(json_string: &str) -> Result<usize, DrawingListError> {
    let bytes = json_string.as_bytes();
    let encoded = bytes
        .strip_prefix(b"\"")
        .and_then(|bytes| bytes.strip_suffix(b"\""))
        .ok_or_else(|| {
            DrawingListError::JsonDecode(
                "encoded asset data must be a Base64 JSON string".to_string(),
            )
        })?;
    if encoded.contains(&b'\\') {
        return Err(DrawingListError::JsonDecode(
            "encoded asset data must not use JSON escapes".to_string(),
        ));
    }
    if encoded.len() % 4 != 0 {
        return Err(DrawingListError::JsonDecode(
            "encoded asset data must use padded Base64".to_string(),
        ));
    }
    let padding = if encoded.ends_with(b"==") {
        2
    } else if encoded.ends_with(b"=") {
        1
    } else {
        0
    };
    let content_len = encoded.len().saturating_sub(padding);
    if encoded[..content_len]
        .iter()
        .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'+' | b'/'))
        || encoded[content_len..].iter().any(|byte| *byte != b'=')
        || (padding == 1 && content_len % 4 != 3)
        || (padding == 2 && content_len % 4 != 2)
    {
        return Err(DrawingListError::JsonDecode(
            "encoded asset data is not canonical Base64".to_string(),
        ));
    }
    (encoded.len() / 4)
        .checked_mul(3)
        .and_then(|decoded| decoded.checked_sub(padding))
        .ok_or_else(|| DrawingListError::invalid("encoded asset byte count overflows usize"))
}

fn validate_extensions(extensions: &BTreeMap<String, Value>) -> Result<(), DrawingListError> {
    if extensions.keys().any(|key| !key.starts_with("x-")) {
        return Err(DrawingListError::invalid(
            "only x-* metadata extensions are forward-compatible",
        ));
    }
    Ok(())
}

/// A bounded writer used by canonical serialization so the output buffer never grows beyond
/// the caller-selected serialized-byte budget.
struct LimitedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    attempted: usize,
    exceeded: bool,
}

impl LimitedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            attempted: 0,
            exceeded: false,
        }
    }

    fn exceeded(&self) -> bool {
        self.exceeded
    }

    fn attempted_bytes(&self) -> usize {
        self.attempted
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for LimitedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.attempted = self.attempted.saturating_add(buffer.len());
        if self.bytes.len().saturating_add(buffer.len()) > self.maximum {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "serialized DrawingList exceeds the configured byte limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn validate_paint(
    paint: Option<&crate::Paint>,
    paint_ids: &BTreeSet<ResourceId>,
) -> Result<(), DrawingListError> {
    if let Some(crate::Paint::Resource { id }) = paint
        && !paint_ids.contains(id)
    {
        return Err(DrawingListError::invalid(format!(
            "paint references unknown paint resource {}",
            id.as_str()
        )));
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
        PathSegment::QuadTo { to, .. }
        | PathSegment::CubicTo { to, .. }
        | PathSegment::ArcTo { to, .. } => Some(*to),
        PathSegment::Close => None,
    }
}
