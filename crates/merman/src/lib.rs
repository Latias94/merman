#![forbid(unsafe_code)]

//! Mermaid parsing and rendering through one operation-scoped facade.
//!
//! [`Renderer`] owns long-lived engine defaults. Each [`RenderRequest`] describes one source,
//! target, resource policy, and [`OperationControl`]. The request is executed synchronously by
//! [`Renderer::render`]; SVG, ASCII, and binary output remain target-local adapters behind the
//! same operation boundary.
//!
//! The facade deliberately does not expose source-to-SVG or source-to-ASCII convenience
//! functions. Keeping the target and its policy in a typed request makes cancellation,
//! deadlines, resource budgets, and output ownership explicit at every call site.
//!
//! # Choosing an API
//!
//! | Goal | Feature | Start with |
//! | --- | --- | --- |
//! | Parse Mermaid or produce semantic JSON | no facade default features | [`Engine`] and [`ParseOptions`] |
//! | Analyze diagnostics or Markdown fences | `analysis` | [`analysis::Analyzer`] |
//! | Build parser-backed editor snapshots | `editor` | [`editor::analyze_document_snapshot_with_shared_text`] |
//! | Render Mermaid-like SVG | `svg` | [`Renderer`] and [`RenderRequest::svg`] |
//! | Render terminal-friendly text | `ascii` | [`Renderer`] and [`RenderRequest::ascii`] |
//! | Render PNG from Rust | `png` | [`Renderer`] and [`RenderRequest::png`] |
//! | Render JPEG from Rust | `jpeg` | [`Renderer`] and [`RenderRequest::jpeg`] |
//! | Render a vector PDF from Rust | `pdf` | [`Renderer`] and [`RenderRequest::pdf`] |
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
//! # Quick start
//!
//! ```no_run
//! # #[cfg(feature = "svg")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
//!
//! let output = Renderer::new().render(RenderRequest::svg(
//!     "flowchart TD\nA[Start] --> B[Done]",
//!     OperationControl::new(),
//!     SvgRequest::default(),
//! ))?;
//! let RenderOutput::Svg(Some(svg)) = output else {
//!     return Err("source did not contain a Mermaid diagram".into());
//! };
//! println!("{}", svg.svg());
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "svg"))]
//! # fn main() {}
//! ```
//!
//! For semantic inspection, use [`Renderer::prepare_semantic`] or a
//! [`RenderTarget::Semantic`] request. For terminal output, use [`RenderTarget::Ascii`]. The
//! target adapters never create a replacement operation or silently replace the caller's
//! cancellation handle.

pub use merman_core::*;

pub mod diagnostic;
#[path = "operation.rs"]
mod operation_runner;
pub mod render;
pub use diagnostic::{
    TerminalDiagnostic, TerminalDiagnosticDetails, TerminalRuntimePolicyError,
    normalize_terminal_diagnostic, normalize_terminal_text,
};
#[cfg(feature = "ascii")]
pub use render::AsciiRequest;
#[cfg(feature = "jpeg")]
pub use render::JpegRequest;
#[cfg(feature = "png")]
pub use render::PngRequest;
#[cfg(any(feature = "png", feature = "jpeg"))]
pub use render::RasterOutput;
#[cfg(feature = "svg")]
pub use render::{
    OperationExecutionPath, RenderEvidence, SvgEnvironment, SvgLayoutOutput, SvgOutput, SvgRequest,
};
#[cfg(feature = "pdf")]
pub use render::{PdfOutput, PdfRequest};
pub use render::{
    RenderError, RenderOutput, RenderRequest, RenderTarget, Renderer, ResourceLimitCause,
    ResourceLimitExceeded, SemanticArtifact,
};

/// Diagnostics and source-mapping APIs for lint and analysis workflows.
#[cfg(feature = "analysis")]
pub use merman_analysis as analysis;

/// Parser-backed editor intelligence APIs.
#[cfg(feature = "editor")]
pub use merman_editor_core as editor;

/// SVG target-local types and backend capabilities.
#[cfg(feature = "svg")]
pub mod svg;

/// ASCII target-local types and model-level backend interface.
#[cfg(feature = "ascii")]
pub mod ascii;
