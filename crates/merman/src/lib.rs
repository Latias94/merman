#![forbid(unsafe_code)]

//! Headless, parity-focused Mermaid parsing and rendering in Rust.
//!
//! `merman` is the public Rust facade for the project. It re-exports
//! [`merman_core`] for detection, parsing, metadata, semantic JSON, and typed
//! render models, then adds optional convenience modules for SVG, binary export,
//! and terminal text output.
//!
//! The compatibility target is Mermaid `@11.16.1`. Upstream Mermaid behavior is
//! treated as the specification, including cases where the browser implementation
//! is surprising. The root README and `docs/alignment/STATUS.md` document the
//! current parity matrix, deferred residuals, and release gates.
//!
//! # Choosing an API
//!
//! | Goal | Feature | Start with |
//! | --- | --- | --- |
//! | Parse Mermaid or produce semantic JSON | no facade default features | [`Engine`] and [`ParseOptions`] |
//! | Analyze diagnostics or Markdown fences | `analysis` | [`analysis::Analyzer`] |
//! | Build parser-backed editor snapshots | `editor` | [`editor::DocumentWorkspace`] |
//! | Render Mermaid-like SVG | `svg` | [`render_svg`] |
//! | Prepare SVG for export | `svg` | `HeadlessRenderer::render_resvg_compatible_svg_sync` |
//! | Render terminal-friendly text | `ascii` | `merman::ascii::HeadlessAsciiRenderer` |
//! | Render PNG from Rust | `png` | `HeadlessRenderer::render_png_sync` and `svg::export::RasterOptions` |
//! | Render JPEG from Rust | `jpeg` | `HeadlessRenderer::render_jpeg_sync` and `svg::export::RasterOptions` |
//! | Render a vector PDF from Rust | `pdf` | `HeadlessRenderer::render_pdf_with_options_sync` and `svg::export::PdfOptions` |
//!
//! If you already know the diagram type, use the `*_with_type_sync` methods on
//! [`Engine`] to skip detection. If you need lower-level layout or SVG pipeline
//! control, use the re-exported types under `merman::svg` or depend on
//! `merman-render` directly.
//!
//! # Features
//!
//! - `analysis`: render-free diagnostics, source mapping, and Markdown analysis through
//!   `merman::analysis`.
//! - `editor`: parser-backed editor snapshots and queries through `merman::editor`; this implies
//!   `analysis`.
//! - `svg`: layout plus SVG rendering through `merman::svg`.
//! - `ascii`: ASCII/Unicode text rendering through `merman::ascii`.
//! - `png`, `jpeg`, and `pdf`: bounded binary export through `merman::svg::export`; each
//!   implies `svg` but does not imply either of the other binary formats.
//! - `math`: pure-Rust math label rendering for the SVG path; this implies
//!   `svg`.
//!
//! The default feature set is [`complete-svg`](#features): it supports complete deterministic SVG
//! rendering, both optional layout engines, and math labels without compiling ambient system
//! adapters. Use `default-features = false` with the direct capability leaves when you need a
//! measured artifact closure.
//!
//! Parser-only applications should depend on `merman-core` directly. If they need this facade's
//! re-exports instead, they must set `default-features = false`; an ordinary `merman` dependency
//! intentionally compiles the complete SVG workflow.
//!
//! # SVG quickstart
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use merman::render_svg;
//!
//! let svg = render_svg("flowchart TD\nA[Start] --> B[Done]")?;
//!
//! println!("{svg}");
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "svg"))]
//! # fn main() {}
//! ```
//!
//! A fresh `HeadlessRenderer` keeps the Mermaid parity SVG contract for
//! `HeadlessRenderer::render_svg_sync`. Calling `with_svg_pipeline` installs a
//! renderer-owned output pipeline for that method. Product presentation is selected
//! independently with `HeadlessRenderer::with_presentation`. Use
//! `HeadlessRenderer::render_svg_readable_sync` when browser
//! `<foreignObject>` labels may need readable `<text>` fallbacks, and
//! `HeadlessRenderer::render_resvg_compatible_svg_sync` when the output will be
//! consumed by `merman-export` or another validated SVG consumer. Use the
//! `*_with_pipeline_sync` variant only when the host also owns custom draft passes.
//!
//! These output choices do not make one universal "safe SVG" promise. The
//! [`MermaidConfig`] `securityLevel` key controls source, label, and navigation
//! URL sanitization. Parity and readable SVG preserve Mermaid navigation metadata;
//! the resvg-safe pipeline closes automatic rendering-resource capabilities for
//! raster and PDF consumers but is not a browser DOM sanitizer. A browser host
//! must separately validate the SVG for its intended policy, such as a closed
//! self-contained preview or an authoring surface with user-activated links.
//! Likewise, Mermaid's `sandbox` security level cannot create an iframe, origin,
//! process, or CSP boundary in this headless Rust library; isolation remains a
//! host responsibility.
//!
//! # ASCII quickstart
//!
//! ```no_run
//! # #[cfg(feature = "ascii")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use merman::ascii::{AsciiRenderOptions, HeadlessAsciiRenderer};
//!
//! let renderer = HeadlessAsciiRenderer::new()
//!     .with_strict_parsing()
//!     .with_ascii_options(AsciiRenderOptions::unicode());
//! let text = renderer
//!     .render_ascii_sync("sequenceDiagram\nA->>B: Hello")?
//!     .expect("diagram detected");
//!
//! println!("{text}");
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "ascii"))]
//! # fn main() {}
//! ```
//!
//! Text output is intentionally terminal-native rather than SVG-derived. The
//! currently supported public subset covers flowchart/graph, sequenceDiagram,
//! classDiagram, erDiagram, stateDiagram, xychart, mindmap, treeView,
//! timeline, gantt, journey, kanban, packet, and gitGraph.
//!
//! # SVG, raster, and PDF output
//!
//! SVG output remains vector markup: Merman does not rasterize it and does not impose a global
//! width/height cap on the SVG root. The normal source, layout, and SVG-byte resource budgets
//! still apply before an SVG is returned.
//!
//! PNG and JPG are pixel outputs. Their `RasterOptions` plan the final pixmap before allocation,
//! with default 4096-by-4096 side limits and a 4096-squared pixel limit. Use
//! `RasterOptions::with_fit_to` for a preview-sized target and `RasterOptions::with_scale` for
//! device-pixel ratio. Embedded raster images are checked from their headers before decoding as
//! well.
//!
//! PDF is a vector output with an independent `PdfOptions` policy. The default
//! `PdfPagePolicy::FitSvg` uses the SVG's intrinsic page size and is not constrained by the
//! PNG/JPG pixmap budget. PDF filters may create localized bitmaps and embedded raster images have
//! their own default budgets; configure those through `PdfOptions` when needed. Use
//! `PdfPagePolicy::FitCssWidth` to model the CSS-pixel viewport used by browser-style `--pdfFit`
//! exports.

pub use merman_core::*;

/// Error from the one-shot [`render_svg`] facade.
///
/// [`NoDiagram`](Self::NoDiagram) distinguishes ordinary prose or an empty
/// input from Mermaid parse, layout, render, resource, and missing-capability
/// failures. Configure a [`svg::HeadlessRenderer`] directly when an operation
/// needs a custom diagram id, runtime policy, resource policy, or SVG
/// pipeline.
#[cfg(feature = "svg")]
#[derive(Debug, thiserror::Error)]
pub enum RenderSvgError {
    #[error("no Mermaid diagram detected")]
    NoDiagram,
    #[error(transparent)]
    Headless(#[from] svg::HeadlessError),
}

/// Renders one Mermaid source string to a standalone parity-oriented SVG.
///
/// This is the shortest Rust entry point for the normal deterministic SVG
/// workflow. It uses the facade's default engine and render environment, and
/// returns [`RenderSvgError::NoDiagram`] when `source` does not contain a
/// Mermaid diagram. Enable the existing `svg` feature to use this API; the
/// default `complete-svg` feature includes it.
///
/// The default SVG id is intended for a standalone output. Use
/// [`render_svg_with_id`] when multiple rendered diagrams share one document.
/// For other request-level configuration, reuse, or a non-default output
/// pipeline, use [`svg::HeadlessRenderer`] instead.
#[cfg(feature = "svg")]
pub fn render_svg(source: &str) -> std::result::Result<String, RenderSvgError> {
    finish_one_shot_svg(svg::HeadlessRenderer::new().render_svg_sync(source))
}

/// Renders one Mermaid source string with an explicit document-unique SVG id.
///
/// Use this entry point when multiple outputs will be embedded in the same DOM.
/// The id is normalized with the same rules as
/// [`svg::HeadlessRenderer::with_diagram_id`].
/// Callers must ensure supplied ids remain unique after [`svg::sanitize_svg_id`] normalization because distinct display labels can normalize to the same emitted id.
#[cfg(feature = "svg")]
pub fn render_svg_with_id(
    source: &str,
    diagram_id: &str,
) -> std::result::Result<String, RenderSvgError> {
    finish_one_shot_svg(
        svg::HeadlessRenderer::new()
            .with_diagram_id(diagram_id)
            .render_svg_sync(source),
    )
}

#[cfg(feature = "svg")]
fn finish_one_shot_svg(
    result: std::result::Result<Option<String>, svg::HeadlessError>,
) -> std::result::Result<String, RenderSvgError> {
    match result {
        Ok(Some(svg)) => Ok(svg),
        Ok(None) | Err(svg::HeadlessError::Parse(merman_core::Error::DetectType(_))) => {
            Err(RenderSvgError::NoDiagram)
        }
        Err(error) => Err(error.into()),
    }
}

/// Diagnostics and source-mapping APIs for lint and analysis workflows.
#[cfg(feature = "analysis")]
pub use merman_analysis as analysis;

/// Parser-backed editor intelligence APIs.
#[cfg(feature = "editor")]
pub use merman_editor_core as editor;

#[cfg(feature = "ascii")]
pub mod ascii;

#[cfg(feature = "svg")]
pub mod svg;
