//! Source callers use the shared markup sanitizer.
//!
//! These forwarding paths remain only while consumers migrate to `merman-markup`.

pub use merman_markup::sanitize::{
    SanitizeFailure, SanitizeOutputSink, remove_script, sanitize_text, sanitize_text_or_array,
    sanitize_text_with_sink, sanitize_xhtml, sanitize_xhtml_in_svg_context,
    sanitize_xhtml_in_svg_context_with_sink, sanitize_xhtml_with_sink,
};

pub(crate) use merman_markup::sanitize::{
    dompurify_normalize_dom_attribute_value, dompurify_normalize_dom_uri_attribute,
    dompurify_normalize_serialized_uri_attribute,
};

#[cfg(all(
    feature = "presentation",
    any(feature = "diagram-flowchart", feature = "diagram-swimlane")
))]
pub(crate) use merman_markup::sanitize::sanitize_text_as_html_fragment;
