#![forbid(unsafe_code)]

//! Headless, parity-focused Mermaid parsing and rendering in Rust.
//!
//! `merman` is the public Rust facade for the project. It re-exports
//! [`merman_core`] for detection, parsing, metadata, semantic JSON, and typed
//! render models, then adds optional convenience modules for SVG, binary export,
//! and terminal text output.
//!
//! The compatibility target is Mermaid `@11.16.0`. Upstream Mermaid behavior is
//! treated as the specification, including cases where the browser implementation
//! is surprising. The root README and `docs/alignment/STATUS.md` document the
//! current parity matrix, deferred residuals, and release gates.
//!
//! # Choosing an API
//!
//! | Goal | Feature | Start with |
//! | --- | --- | --- |
//! | Parse Mermaid or produce semantic JSON | none | [`Engine`] and [`ParseOptions`] |
//! | Render Mermaid-like SVG | `svg` | `merman::svg::HeadlessRenderer` |
//! | Prepare SVG for export | `svg` | `HeadlessRenderer::render_resvg_compatible_svg_with_pipeline_sync` |
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
//! - `svg`: layout plus SVG rendering through `merman::svg`.
//! - `ascii`: ASCII/Unicode text rendering through `merman::ascii`.
//! - `png`, `jpeg`, and `pdf`: bounded binary export through `merman::svg::export`; each
//!   implies `svg` but does not imply either of the other binary formats.
//! - `math`: pure-Rust math label rendering for the SVG path; this implies
//!   `svg`.
//!
//! The default feature set is [`complete-svg`](#features): it supports normal native SVG
//! rendering, both optional layout engines, and math labels without compiling ambient system
//! adapters. Use `default-features = false` with the direct capability leaves when you need a
//! measured artifact closure.
//!
//! # SVG quickstart
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use merman::svg::HeadlessRenderer;
//!
//! let renderer = HeadlessRenderer::new().with_diagram_id("readme-example");
//! let svg = renderer
//!     .render_svg_sync("flowchart TD\nA[Start] --> B[Done]")?
//!     .expect("diagram detected");
//!
//! println!("{svg}");
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "svg"))]
//! # fn main() {}
//! ```
//!
//! A fresh `HeadlessRenderer` keeps the Mermaid parity SVG contract for
//! `HeadlessRenderer::render_svg_sync`. Calling `with_host_theme` or
//! `with_svg_pipeline` installs a renderer-owned output pipeline for that
//! method. Use
//! `HeadlessRenderer::render_svg_readable_sync` when browser
//! `<foreignObject>` labels may need readable `<text>` fallbacks, and
//! `HeadlessRenderer::render_svg_resvg_safe_sync` when the output will
//! be consumed by `merman-export` or another validated SVG consumer.
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
#[cfg(feature = "svg")]
pub use svg::supported_host_theme_presets;
