//! ASCII target-local types and the model-level backend interface.
//!
//! Source parsing and operation ownership belong to [`crate::Renderer`]. This module intentionally
//! does not provide a second source-to-text orchestration layer. Hosts that already own a typed
//! [`merman_core::diagram::RenderSemanticModel`] may use [`render_model`] directly; normal source
//! rendering should use [`crate::RenderRequest::ascii`].

pub use merman_ascii::{
    ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiCapability, AsciiCapabilityEvidence, AsciiCharset,
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiDirection, AsciiError, AsciiEvidenceKind,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitDescriptor, AsciiResourcePolicy, AsciiRgb,
    AsciiSupportLevel, AsciiTerminalPalette, MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID,
    ascii_capabilities, ascii_resource_profile_value, ascii_supported_diagram_types, render_model,
    render_model_with_local_time_zone, render_model_with_operation,
};
