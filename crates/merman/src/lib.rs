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

#[path = "operation.rs"]
mod operation_runner;
pub mod render;
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
