#![forbid(unsafe_code)]
//! Versioned, renderer-neutral drawing-list contracts for Merman.
//!
//! This crate deliberately contains no Mermaid parser, layout, SVG, raster/PDF, FFI, or host
//! graphics backend dependency. It owns the public document model and validation boundary shared
//! by Merman renderers and cross-language consumers.

mod commands;
mod document;
mod error;
mod geometry;
mod resources;
mod validation;
mod wire;

pub use commands::{
    BlendMode, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, LineCap,
    LineJoin, MeasurementProvenance, PathStyle, PositionedGlyph, StrokeStyle, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextPaintOrder, TextRun, TextStyle,
};
pub use document::{
    AlphaMode, CoordinateSystem, DrawingListDocument, DrawingListFootprint, DrawingListLimits,
    FallbackReason, RasterFallback, RasterFormat, SemanticAnnotation, SemanticRole, Viewport,
    VisualSource,
};
pub use error::DrawingListError;
pub use geometry::{Color, Point, Rect, Transform};
pub use resources::{
    DrawingResource, EncodedAsset, EncodedImage, FontResource, GradientSpread, GradientStop,
    ImagePatternResource, ImageResource, LinearGradientResource, Paint, PathResource, PathSegment,
    PatternRepeat, PatternResource, RadialGradientResource, ResourceId,
};
pub use validation::ValidationEvent;

/// The current public DrawingList wire schema version.
pub const DRAWING_LIST_VERSION: u32 = 1;

/// Maximum nested object/array depth inside each metadata extension value.
///
/// Scalars have depth zero. The document and extensions objects use two additional levels,
/// keeping canonical output within serde_json's default recursion limit.
pub const DRAWING_LIST_MAX_EXTENSION_DEPTH: usize = 125;

/// The media type returned by the generic binding operation.
pub const DRAWING_LIST_MEDIA_TYPE: &str = "application/vnd.merman.drawing-list+json;version=1";
