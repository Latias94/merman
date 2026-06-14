#![forbid(unsafe_code)]

//! Safe shared facade used by external binding crates.
//!
//! This crate owns options parsing, renderer setup, result-code classification, and byte payload
//! production. Unsafe transport concerns such as raw pointers and owned C buffers remain in
//! `merman-ffi`.

mod common;
mod engine;
mod metadata;

#[cfg(feature = "ascii")]
mod ascii;
#[cfg(feature = "render")]
mod render;

pub use common::{
    BindingError, BindingStatus, error_payload_json_bytes, render_payload_json_bytes,
};
pub use engine::BindingEngine;
pub use metadata::{
    BindingCapabilities, BindingDiagramFamilyCapability, ascii_supported_diagrams,
    ascii_supported_diagrams_json, binding_capabilities, diagram_family_capabilities,
    diagram_family_capabilities_json, selected_registry_profile, supported_diagrams,
    supported_diagrams_json, supported_host_theme_presets, supported_host_theme_presets_json,
    supported_themes, supported_themes_json,
};

#[cfg(feature = "ascii")]
pub use ascii::render_ascii;
#[cfg(feature = "render")]
pub use render::{layout_json, parse_json, render_svg, validate_json};

#[cfg(not(feature = "ascii"))]
pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let _ = (source, options_json);
    Err(common::feature_required_error("ASCII rendering", "ascii"))
}

#[cfg(not(feature = "render"))]
pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let _ = (source, options_json);
    Err(common::feature_required_error("SVG rendering", "render"))
}

#[cfg(not(feature = "render"))]
pub fn parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let _ = (source, options_json);
    Err(common::feature_required_error("parse_json", "render"))
}

#[cfg(not(feature = "render"))]
pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let _ = (source, options_json);
    Err(common::feature_required_error("layout_json", "render"))
}

#[cfg(not(feature = "render"))]
pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let _ = (source, options_json);
    common::validation_payload_json(Err(common::feature_required_error("validation", "render")))
}

#[cfg(all(test, any(not(feature = "render"), not(feature = "ascii"))))]
mod tests {
    use super::*;
    #[cfg(not(feature = "render"))]
    use serde_json::Value;

    #[cfg(not(feature = "render"))]
    #[test]
    fn render_entry_points_report_missing_render_feature() {
        let err = render_svg(b"flowchart TD\nA", b"").unwrap_err();
        assert_eq!(err.status(), BindingStatus::UnsupportedFormat);
        assert!(err.message().contains("render feature"));

        let err = parse_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_eq!(err.status(), BindingStatus::UnsupportedFormat);

        let err = layout_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_eq!(err.status(), BindingStatus::UnsupportedFormat);

        let validation: Value =
            serde_json::from_slice(&validate_json(b"flowchart TD\nA", b"").unwrap()).unwrap();
        assert_eq!(validation["valid"], false);
        assert_eq!(
            validation["code_name"],
            BindingStatus::UnsupportedFormat.code_name()
        );
    }

    #[cfg(not(feature = "ascii"))]
    #[test]
    fn ascii_entry_point_reports_missing_ascii_feature() {
        let err = render_ascii(b"flowchart TD\nA", b"").unwrap_err();
        assert_eq!(err.status(), BindingStatus::UnsupportedFormat);
        assert!(err.message().contains("ascii feature"));
    }
}
