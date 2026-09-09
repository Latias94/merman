use thiserror::Error;

/// Errors produced while validating or decoding a DrawingList document.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DrawingListError {
    #[error("unsupported DrawingList version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("invalid DrawingList document: {0}")]
    InvalidDocument(String),
    #[error("DrawingList resource limit exceeded for {resource}: {actual} > {maximum}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("DrawingList JSON decode failed: {0}")]
    JsonDecode(String),
    #[error("DrawingList JSON encode failed: {0}")]
    JsonEncode(String),
}

impl DrawingListError {
    /// Returns the caller-facing limit field for a protocol resource rejection.
    pub fn limit_id(&self) -> Option<&'static str> {
        let Self::ResourceLimit { resource, .. } = self else {
            return None;
        };
        Some(match *resource {
            "serialized_bytes" => "max_serialized_bytes",
            "commands" => "max_commands",
            "resources" => "max_resources",
            "path_segments" => "max_path_segments",
            "stroke_dash_entries" => "max_stroke_dash_entries",
            "image_bytes" => "max_image_bytes",
            "image_pixels" => "max_image_pixels",
            "fallback_pixels" => "max_fallback_pixels",
            "font_bytes" => "max_font_bytes",
            "nesting_depth" => "max_nesting_depth",
            "fallbacks" => "max_fallbacks",
            "text_bytes" => "max_text_bytes",
            "glyphs" => "max_glyphs",
            unknown => unknown,
        })
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidDocument(message.into())
    }
}
