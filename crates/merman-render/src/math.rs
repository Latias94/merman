//! Optional math rendering hooks.
//!
//! Upstream Mermaid renders `$$...$$` fragments via KaTeX and measures the resulting HTML in a
//! browser DOM. merman is headless and pure-Rust by default, so math rendering is modeled as an
//! optional, pluggable backend.
//!
//! This module only defines an interface; the default implementation is a no-op.

use crate::text::{TextMetrics, TextStyle, WrapMode};
use merman_core::MermaidConfig;

/// Optional math renderer used to transform label HTML and (optionally) provide measurements.
///
/// Implementations should be:
/// - deterministic (stable output across runs),
/// - side-effect free (no global mutations),
/// - non-panicking (return `None` to decline handling).
pub trait MathRenderer: std::fmt::Debug {
    /// Attempts to render math fragments within an HTML label string.
    ///
    /// If the renderer declines to handle the input, it should return `None`.
    ///
    /// The returned string is treated as raw HTML and will still be sanitized by merman before
    /// emitting into an SVG `<foreignObject>`.
    fn render_html_label(&self, text: &str, config: &MermaidConfig) -> Option<String>;

    /// Optionally measures the rendered HTML label in pixels.
    ///
    /// This is intended to mirror upstream Mermaid's DOM measurement behavior for math labels.
    /// The default implementation returns `None`.
    fn measure_html_label(
        &self,
        _text: &str,
        _config: &MermaidConfig,
        _style: &TextStyle,
        _max_width_px: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        None
    }
}

/// Default math renderer: does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMathRenderer;

impl MathRenderer for NoopMathRenderer {
    fn render_html_label(&self, _text: &str, _config: &MermaidConfig) -> Option<String> {
        None
    }
}
