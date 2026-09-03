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

pub use commands::{
    BlendMode, DrawingCommand, DrawingListPolicy, FontDescriptor, FontStyle, LineCap, LineJoin,
    MeasurementProvenance, PathStyle, StrokeStyle, TextObligation, TextRun, TextStyle,
};
pub use document::{
    CoordinateSystem, DrawingListDocument, DrawingListLimits, FallbackReason, RasterFallback,
    SemanticAnnotation, SemanticRole, Viewport, VisualSource,
};
pub use error::DrawingListError;
pub use geometry::{Color, Point, Rect, Transform};
pub use resources::{
    DrawingResource, EncodedImage, GradientStop, ImageResource, LinearGradientResource, Paint,
    PathResource, PathSegment, ResourceId,
};

/// The current public DrawingList wire schema version.
pub const DRAWING_LIST_VERSION: u32 = 1;

/// The media type returned by the generic binding operation.
pub const DRAWING_LIST_MEDIA_TYPE: &str = "application/vnd.merman.drawing-list+json;version=1";
