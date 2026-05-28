//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! This module is named `parity` to reflect intent: upstream Mermaid is treated as the spec, and
//! SVG output is gated by DOM parity checks.

#![forbid(unsafe_code)]

mod fallback;
mod parity;
mod pipeline;

pub use fallback::foreign_object_label_fallback_svg_text;
pub use parity::*;
pub use pipeline::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor,
    StripForeignObjectPostprocessor, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext,
    SvgPostprocessMetadata, SvgPostprocessor, resvg_safe_svg,
};
