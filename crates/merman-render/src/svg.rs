//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! This module is named `parity` to reflect intent: upstream Mermaid is treated as the spec, and
//! SVG output is gated by DOM parity checks.

#![forbid(unsafe_code)]

mod fallback;
mod icon_registry;
mod parity;
mod pipeline;
pub(crate) mod scanner;
mod theme_profile;

#[cfg(feature = "layout-cytoscape")]
pub(crate) use parity::render_architecture_family_artifact;
pub(crate) use parity::render_builtin_family_artifact;
pub(crate) use parity::theme as render_theme;

pub use fallback::foreign_object_label_fallback_svg_text;
pub use icon_registry::{IconRegistry, IconRegistryError, IconSvg};
pub use parity::*;
pub use pipeline::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    ResvgCompatibleSvg, RootBackgroundPostprocessor, SanitizeCssPostprocessor,
    SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor, StripForeignObjectPostprocessor,
    SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext, SvgPostprocessMetadata,
    SvgPostprocessor, finalize_resvg_svg,
};
pub use theme_profile::{
    CompiledHostTheme, HostThemeAppearance, HostThemeOutput, HostThemePipelinePreset,
    HostThemePreset, HostThemeProfile, HostThemeProfileBuilder, HostThemeRoles,
    HostThemeRootBackground, supported_host_theme_presets,
};
